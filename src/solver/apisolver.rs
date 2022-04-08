//! This module performs actions using MediaWiki API
//! 

use super::{util, error::SolveError};
use std::collections::{HashSet, VecDeque};
use mediawiki::{api::NamespaceID, title::Title, hashmap};
use crate::API_SERVICE;
use crate::parser::ir::{DepthNum, RedirectFilterStrategy};

fn limit_to_max(limit: i64) -> Option<usize> {
    if limit < 0 {
        None
    } else {
        Some(limit as usize)
    }
}

fn pages_object_to_titles_set(data: &serde_json::Value, redirected: bool, redirect_filter: RedirectFilterStrategy) -> HashSet<Title> {
    if let Some(obj) = data.as_object() {
        let mut redirects: HashSet<Title> = HashSet::new();
        if let Some(redirs) = obj.get("redirects") {
            for itm in redirs.as_array().unwrap().iter() {
                redirects.insert(API_SERVICE.title_new_from_full(itm["from"].as_str().unwrap()).unwrap());
            }
        }
        let mut pages: HashSet<Title> = HashSet::new();
        if let Some(pgs) = obj.get("pages") {
            for pageobj in pgs.as_array().unwrap() {
                pages.insert(Title::new_from_api_result(pageobj));
            }
        }
        if redirected {
            match redirect_filter {
                RedirectFilterStrategy::NoRedirect => pages,
                RedirectFilterStrategy::OnlyRedirect => redirects,
                RedirectFilterStrategy::All => redirects.union(&pages).cloned().collect(),
            }
        } else {
            pages
        }
    } else {
        HashSet::new()
    }
}

/// Retrives the backlink for one page.
/// 
/// "Backlink" refers to internal links and redirects. Transclusions (common for templates) are not considered as backlinks.
/// For example, if template B has a link to page A, and template B is transcluded into page C, then C is a backlink to A but B is not.
/// 
/// `title`: The title of the page.
/// 
/// `api`: The MediaWiki API instance.
/// 
/// `assert`: The identity to assert for when using MediaWiki API. If set to `None`, won't apply assertion.
/// 
/// `ns`: Namespace filter. If set to `None`, then the result is not filtered by namespace.
/// 
/// `level_2`: Whether to include pages that links to a redirect of `title`.
/// 
/// `redirect_strat`: The redirect strategy to use when querying.
/// 
/// `follow_redir`: Whether should follow redirects. Usually you don't want to do this, because the redirects returned from this function all link to the page you are querying.
/// 
/// `limit`: Query limit.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn get_backlinks_one(title: &Title, ns: Option<&HashSet<NamespaceID>>, level_2: bool, redirect_strat: RedirectFilterStrategy, follow_redir: bool, limit: i64) -> Result<HashSet<Title>, SolveError> {
    let elem_name = API_SERVICE.full_pretty(title)?;
    if elem_name.is_none() {
        Ok(HashSet::new())
    } else {
        let mut params = hashmap![
            "action".to_string() => "query".to_string(),
            "generator".to_string() => "backlinks".to_string(),
            "gbltitle".to_string() => elem_name.unwrap(),
            "gbllimit".to_string() => "max".to_string(),
            "gblfilterredir".to_string() => redirect_strat.to_string()
        ];
        if follow_redir {
            params.insert("redirects".to_string(), "1".to_string());
        }
        if level_2 {
            // If level_2 is `true`, we cannot filter namespaces in the query. Here is the reason.
            // Suppose we have an inter-namespace redirect, for example,
            // [[w:zh:LTA:KAGE]] (main) -> [[w:zh:Wikipedia:持续出没的破坏者/User:影武者]] (Project)
            // and there are pages in "Project" namespace that link to [[LTA:KAGE]].
            // If we add "blnamespace=4" ("Project")  to the query, we cannot access these pages,
            // because the link target [[LTA:KAGE]] (main) is filtered out.
            params.insert("gblredirect".to_string(), "1".to_string());
        } else {
            // We can safely apply namespace restrictions
            if let Some(ns_list) = ns {
                params.insert("gblnamespace".to_string(), util::concat_params(ns_list));
            }
        }
        let res = API_SERVICE.get_limit(&params, limit_to_max(limit)).await?;
        let mut title_set = pages_object_to_titles_set(&res["query"], follow_redir, redirect_strat);
        // Need to filter by namespace...
        if level_2 {
            if let Some(ns_list) = ns {
                title_set.retain(|title| ns_list.contains(&title.namespace_id()));
            }
        }
        Ok(title_set)
    }
}

/// Retrives the members of one category. Dive into subcategories if possible.
/// Unfortunately, MediaWiki API does not provide any option to filter out redirects.
/// 
/// `title`: The title of the category.
/// 
/// `api`: The MediaWiki API instance.
/// 
/// `assert`: The identity to assert for when using MediaWiki API. If set to `None`, won't apply assertion.
/// 
/// `ns`: Namespace filter. If set to `None`, then the result is not filtered by namespace.
/// 
/// `depth`: Maximum depth we should dive into. The category `title` sits at level 0, its sub categories sit at level 1, and so on. If `depth` is negative, then **every subcategory** in the hierarchy will be visited, which could be costly.
/// 
/// `follow_redir`: Whether should follow redirects.
/// 
/// `limit`: Query limit.
pub(crate) async fn get_category_members_one(title: &Title, ns: Option<&HashSet<NamespaceID>>, depth: DepthNum, follow_redir: bool, limit: i64) -> Result<HashSet<Title>, SolveError> {
    // Due to miser mode, we need to do some preparations to cs.
    let mut ns_clone = ns.cloned();
    let mut result_has_ns_category: bool = true;
    let mut result_has_ns_file: bool = true;
    if let Some(ns_list) = ns_clone.as_mut() {
        result_has_ns_category = ns_list.remove(&super::def::NS_CATEGORY);
        result_has_ns_file = ns_list.remove(&super::def::NS_FILE);
    }
    // Do a bfs search of category tree (perhaps graph).
    // Looks like it is possible to construct a "sub category loop".
    // In fact, [[w:en:Category:Recursion]] is indef full protected to
    // prevent editors from adding itself to its sub categories.
    let mut result_set: HashSet<Title> = HashSet::new();
    let mut visited_cats: HashSet<Title> = HashSet::new();
    visited_cats.insert(title.to_owned());
    let mut visit_cat_queue: VecDeque<(Title, DepthNum)> = VecDeque::new();
    visit_cat_queue.push_back((title.to_owned(), 0));
    while let Some((this_cat, this_depth)) = visit_cat_queue.pop_front() {
        if this_cat.namespace_id() != super::def::NS_CATEGORY {
            return Err(SolveError::NotCategory);
        }
        let cat_name = API_SERVICE.full_pretty(&this_cat)?.unwrap();
        let mut params = hashmap![
            "action".to_string() => "query".to_string(),
            "generator".to_string() => "categorymembers".to_string(),
            "gcmtitle".to_string() => cat_name,
            "gcmlimit".to_string() => "max".to_string()
        ];
        if follow_redir {
            params.insert("redirects".to_string(), "1".to_string());
        }
        // determine what cmtype and cmnamespace should we insert
        let mut cmtype: Vec<String> = Vec::new();
        let mut cmnamespace: HashSet<NamespaceID> = HashSet::new();
        // If we still have some namespaces left in `ns_clone`...
        if let Some(ns_list) = &ns_clone {
            if ns_list.len() > 0 {
                cmtype.push("page".to_string());
                cmnamespace.extend(ns_list);
            }
        } else {
            cmtype.push("page".to_string());
        }
        if result_has_ns_file {
            cmtype.push("file".to_string());
            cmnamespace.insert(super::def::NS_FILE);
        }
        // If we still need to find subcats, or `result_has_ns_category`...
        if result_has_ns_category || (depth < 0 || this_depth < depth) {
            cmtype.push("subcat".to_string());
            cmnamespace.insert(super::def::NS_CATEGORY);
        }
        if ns_clone.is_some() {
            params.insert("gcmnamespace".to_string(), util::concat_params(&cmnamespace));
        }
        params.insert("gcmtype".to_string(), cmtype.join("|"));
        // fetch results
        let res = API_SERVICE.get_limit(&params, limit_to_max(limit)).await?;
        let mut title_set_2 = pages_object_to_titles_set(&res["query"], follow_redir, RedirectFilterStrategy::NoRedirect);
        if depth < 0 || this_depth < depth {
            // filter out subcategories from title_vec, and add to visit queue
            for sub in title_set_2.iter().filter(|&t| t.namespace_id() == super::def::NS_CATEGORY) {
                if !visited_cats.contains(sub) {
                    visited_cats.insert(sub.to_owned());
                    visit_cat_queue.push_back((sub.to_owned(), this_depth + 1));
                }
            }
        }
        if !result_has_ns_category {
            title_set_2.retain(|f| f.namespace_id() != super::def::NS_CATEGORY);
        }
        result_set.extend(title_set_2);
    }
    Ok(result_set)
}

/// Retrives the pages with the given prefix. That is how [[Special:PrefixIndex]] works.
/// 
/// This function does not need a namespace constraint, because it is implied by the prefix.
/// However, we still provide it. If the page's namespace does not exist in the requested namespaces,
/// an empty set is directly returned without any API requests.
/// 
/// Also, MediaWiki API prohibits the use of redirect resolving when using allpages as a generator, thus `follow_redir` is unavailable.
/// 
/// `title`: The title of the page.
/// 
/// `api`: The MediaWiki API instance.
/// 
/// `assert`: The identity to assert for when using MediaWiki API. If set to `None`, won't apply assertion.
/// 
/// `ns`: Namespace filter. If set to `None`, then the result is not filtered by namespace.
/// 
/// `redirect_strat`: The redirect strategy to use when querying.
/// 
/// `limit`: Query limit.
pub(crate) async fn get_prefix_index_one(title: &Title, ns: Option<&HashSet<NamespaceID>>, redirect_strat: RedirectFilterStrategy, limit: i64) -> Result<HashSet<Title>, SolveError> {
    let title_ns_id = title.namespace_id();
    if let Some(ns_list) = ns {
        if !ns_list.contains(&title_ns_id) {
            return Ok(HashSet::new());
        }
    }
    let params = hashmap![
        "action".to_string() => "query".to_string(),
        "generator".to_string() => "allpages".to_string(),
        "gapprefix".to_string() => title.pretty().to_string(),
        "gapnamespace".to_string() => title_ns_id.to_string(),
        "gaplimit".to_string() => "max".to_string(),
        "gapfilterredir".to_string() => redirect_strat.to_string()
    ];
    let res = API_SERVICE.get_limit(&params, limit_to_max(limit)).await?;
    let title_set = pages_object_to_titles_set(&res["query"], false, redirect_strat);
    Ok(title_set)
}

/// Retrives the pages that embeds a specific page.
/// 
/// Any page that transcludes this page (either via template redirects, or template itself uses this page) is considered embeds this page.
/// 
/// `title`: The title of the page.
/// 
/// `api`: The MediaWiki API instance.
/// 
/// `assert`: The identity to assert for when using MediaWiki API. If set to `None`, won't apply assertion.
/// 
/// `ns`: Namespace filter. If set to `None`, then the result is not filtered by namespace.
/// 
/// `redirect_strat`: The redirect strategy to use when querying. This is useful if a redirect page itself transcludes this page.
/// 
/// `follow_redir`: Whether should follow redirects.
/// 
/// `limit`: Query limit.
pub(crate) async fn get_embed_one(title: &Title, ns: Option<&HashSet<NamespaceID>>, redirect_strat: RedirectFilterStrategy, follow_redir: bool, limit: i64) -> Result<HashSet<Title>, SolveError> {
    let elem_name = API_SERVICE.full_pretty(title)?;
    if elem_name.is_none() {
        Ok(HashSet::new())
    } else {
        let mut params = hashmap![
            "action".to_string() => "query".to_string(),
            "generator".to_string() => "embeddedin".to_string(),
            "geititle".to_string() => elem_name.unwrap(),
            "geilimit".to_string() => "max".to_string(),
            "geifilterredir".to_string() => redirect_strat.to_string()
        ];
        if let Some(ns_list) = ns {
            params.insert("geinamespace".to_string(), util::concat_params(ns_list));
        }
        if follow_redir {
            params.insert("redirects".to_string(), "1".to_string());
        }
        let res = API_SERVICE.get_limit(&params, limit_to_max(limit)).await?;
        let title_set = pages_object_to_titles_set(&res["query"], follow_redir, redirect_strat);
        Ok(title_set)
    }
}

/// Retrives the in-wiki links of a page.
/// 
/// `title`: The title of the page.
/// 
/// `api`: The MediaWiki API instance.
/// 
/// `assert`: The identity to assert for when using MediaWiki API. If set to `None`, won't apply assertion.
/// 
/// `ns`: Namespace filter. If set to `None`, then the result is not filtered by namespace.
/// 
/// `follow_redir`: Whether should follow redirects.
/// 
/// `limit`: Query limit
pub(crate) async fn get_links_one(title: &Title, ns: Option<&HashSet<NamespaceID>>, follow_redir: bool, limit: i64) -> Result<HashSet<Title>, SolveError> {
    let elem_name = API_SERVICE.full_pretty(title)?;
    if elem_name.is_none() {
        Ok(HashSet::new())
    } else {
        let mut params = hashmap![
            "action".to_string() => "query".to_string(),
            "generator".to_string() => "links".to_string(),
            "titles".to_string() => elem_name.unwrap(),
            "gpllimit".to_string() => "max".to_string()
        ];
        if let Some(ns_list) = ns {
            params.insert("gplnamespace".to_string(), util::concat_params(ns_list));
        }
        if follow_redir {
            params.insert("redirects".to_string(), "1".to_string());
        }
        let res = API_SERVICE.get_limit(&params, limit_to_max(limit)).await?;
        let title_vec = pages_object_to_titles_set(&res["query"], follow_redir, RedirectFilterStrategy::NoRedirect);
        let title_set = HashSet::from_iter(title_vec.into_iter());
        Ok(title_set)
    }
}

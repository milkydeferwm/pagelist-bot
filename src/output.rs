use std::cmp::Ordering;
use std::collections::{HashSet, HashMap};
use mediawiki::title::Title;
use mediawiki::page::Page;
use mediawiki::api::Api;
use plbot_base::bot::APIAssertType;
use md5::{Md5, Digest};

pub fn generate_text(list: &HashSet<Title>, api: &Api, before_list_text: &str, list_item_text: &str, between_item_text: &str, after_list_text: &str) -> String {
    let mut output: String = String::new();
    output.push_str(&before_list_text);
    let mut titles_vec: Vec<Title> = Vec::from_iter(list.iter().cloned());
    titles_vec.sort_by(|a, b| {
        if a.namespace_id() < b.namespace_id() {
            Ordering::Less
        } else if a.namespace_id() > b.namespace_id() {
            Ordering::Greater
        } else {
            a.pretty().cmp(&b.pretty())
        }
    });
    let item_str: String = titles_vec.iter().map(|t| substitute_template(t, list_item_text, api)).collect::<Vec<String>>().join(&between_item_text);
    output.push_str(&item_str);
    output.push_str(&after_list_text);
    output
}

fn substitute_template(t: &Title, template: &str, api: &Api) -> String {
    let mut output: String = String::new();
    let mut escape: bool = false;
    for char in template.chars() {
        if escape {
            // only accept $0 (full name), $1 (namespace), $2 (name), $$ ($)
            match char {
                '$' => { output.push('$'); },
                '0' => { output.push_str(&t.full_pretty(api).unwrap_or("".to_string())); },
                '1' => { output.push_str(t.namespace_name(api).unwrap_or("")); },
                '2' => { output.push_str(t.pretty()); },
                _ => { output.push('$'); output.push(char); },
            }
            escape = false;
        } else {
            if char == '$' {
                escape = true;
            } else {
                output.push(char);
            }
        }
    }
    output
}

#[derive(Debug)]
pub enum EditPageError {
    BadTitle,
    MediaWiki(mediawiki::media_wiki_error::MediaWikiError),
    EditError(String, String),
}

impl std::error::Error for EditPageError {}
unsafe impl Send for EditPageError {}

impl std::fmt::Display for EditPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditPageError::BadTitle => f.write_str("bad title"),
            EditPageError::MediaWiki(e) => e.fmt(f),
            EditPageError::EditError(code, info) => f.write_fmt(format_args!("MediaWiki API returns error code: \"{}\", more info: \"{}\"", code, info)),
        }
    }
}

impl From<mediawiki::media_wiki_error::MediaWikiError> for EditPageError {
    fn from(e: mediawiki::media_wiki_error::MediaWikiError) -> Self {
        Self::MediaWiki(e)
    }
}

impl From<(String, String)> for EditPageError {
    fn from(e: (String, String)) -> Self {
        Self::EditError(e.0, e.1)
    }
}

/// Replaces the contents of this `Page` with the given text, using the given
/// edit summary.
/// 
/// This version differs from `Page::edit_text` in `formatversion` and `assert`.
/// Also it uses md5.
/// 
/// 
pub async fn write_page(page: &Page, api: &mut Api, text: impl Into<String>, summary: impl Into<String>, assert: Option<APIAssertType>, bot: bool) -> Result<(), EditPageError> {
    let title = page.title().full_pretty(api).ok_or_else(|| EditPageError::BadTitle)?;
    let text_string = text.into();
    let mut hasher = Md5::new();
    hasher.update(&text_string);
    let result = hasher.finalize();
    let md5 = hex::encode(result);
    let mut params: HashMap<String, String> = [
        ("action", "edit"),
        ("title", &title),
        ("text", &text_string),
        ("summary", &summary.into()),
        ("utf8", "1"),
        ("md5", &md5),
        ("nocreate", "1"),
        ("token", &api.get_edit_token().await?),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();

    if bot && matches!(assert, Some(APIAssertType::Bot)) {
        params.insert("bot".to_string(), "1".to_string());
    }
    if let Some(a) = assert {
        params.insert("assert".to_string(), a.to_string());
    };

    let result = api.post_query_api_json(&params).await?;
    match result["edit"]["result"].as_str() {
        Some("Success") => Ok(()),
        _ => {
            let ecode = result["code"].as_str().unwrap_or("<unknown>");
            let einfo = result["info"].as_str().unwrap_or("<unknown>");
            return Err(EditPageError::from((String::from(ecode), String::from(einfo))));
        },
    }
}

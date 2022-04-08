extern crate mediawiki;

mod util;
mod error;
mod apisolver;
mod def;

pub use error::SolveError;
use crate::{parser::{ir::RegID, ir::RedirectFilterStrategy}, API_SERVICE};
use util::{get_set_1, get_set_2};

use crate::parser::{Query, ir::Instruction};

use std::collections::{HashSet, HashMap};
use mediawiki::{title::Title};

pub(crate) type Register = HashMap<RegID, HashSet<Title>>;

pub async fn solve_api(query: &Query, default_limit: i64) -> Result<HashSet<Title>, SolveError> {
    // prepare a mock register pool using HashMap
    let mut reg: Register = HashMap::new();
    for inst in query.0.iter() {
        match inst {
            Instruction::And { dest, op1, op2 } => {
                let (set1, set2) = get_set_2(&reg, op1, op2)?;
                let intersect: HashSet<Title> = set1.intersection(set2).cloned().collect();
                reg.insert(*dest, intersect);
            },
            Instruction::Or { dest, op1, op2 } => {
                let (set1, set2) = get_set_2(&reg, op1, op2)?;
                let union: HashSet<Title> = set1.union(set2).cloned().collect();
                reg.insert(*dest, union);
            },
            Instruction::Exclude { dest, op1, op2 } => {
                let (set1, set2) = get_set_2(&reg, op1, op2)?;
                let diff: HashSet<Title> = set1.difference(set2).cloned().collect();
                reg.insert(*dest, diff);
            },
            Instruction::Xor { dest, op1, op2 } => {
                let (set1, set2) = get_set_2(&reg, op1, op2)?;
                let xor: HashSet<Title> = set1.symmetric_difference(set2).cloned().collect();
                reg.insert(*dest, xor);
            },
            Instruction::Link { dest, op, cs } => {
                let set = get_set_1(&reg, op)?;
                if set.is_empty() {
                    reg.insert(*dest, HashSet::new());
                } else if set.len() > 1 {
                    return Err(SolveError::QueryForMultiplePages);
                } else {
                    let mut result_set: HashSet<Title> = HashSet::new();
                    for t in set.iter() {
                        let res_one = apisolver::get_links_one(t, cs.ns.as_ref(), cs.resolveredir.unwrap_or(false), cs.limit.unwrap_or(default_limit)).await?;
                        result_set.extend(res_one);
                    }
                    reg.insert(*dest, result_set);
                }
            },
            Instruction::LinkTo { dest, op, cs } => {
                let set = get_set_1(&reg, op)?;
                if set.is_empty() {
                    reg.insert(*dest, HashSet::new());
                } else if set.len() > 1 {
                    return Err(SolveError::QueryForMultiplePages);
                } else {
                    let mut result_set: HashSet<Title> = HashSet::new();
                    for t in set.iter() {
                        let res_one = apisolver::get_backlinks_one(t, cs.ns.as_ref(), !cs.directlink.unwrap_or(false), cs.redir.unwrap_or(RedirectFilterStrategy::All), cs.resolveredir.unwrap_or(false), cs.limit.unwrap_or(default_limit)).await?;
                        result_set.extend(res_one);
                    }
                    reg.insert(*dest, result_set);
                }
            },
            Instruction::EmbeddedIn { dest, op, cs } => {
                let set = get_set_1(&reg, op)?;
                if set.is_empty() {
                    reg.insert(*dest, HashSet::new());
                } else if set.len() > 1 {
                    return Err(SolveError::QueryForMultiplePages);
                } else {
                    let mut result_set: HashSet<Title> = HashSet::new();
                    for t in set.iter() {
                        let res_one = apisolver::get_embed_one(t, cs.ns.as_ref(), cs.redir.unwrap_or(RedirectFilterStrategy::All), cs.resolveredir.unwrap_or(false), cs.limit.unwrap_or(default_limit)).await?;
                        result_set.extend(res_one);
                    }
                    reg.insert(*dest, result_set);
                }
            },
            Instruction::InCat { dest, op, cs } => {
                let set = get_set_1(&reg, op)?;
                if set.is_empty() {
                    reg.insert(*dest, HashSet::new());
                } else if set.len() > 1 {
                    return Err(SolveError::QueryForMultiplePages);
                } else {
                    let sub_limit = cs.depth.unwrap_or(0);
                    let mut result_set: HashSet<Title> = HashSet::new();
                    for t in set.iter() {
                        let res_one = apisolver::get_category_members_one(t, cs.ns.as_ref(), sub_limit, cs.resolveredir.unwrap_or(false), cs.limit.unwrap_or(default_limit)).await?;
                        result_set.extend(res_one);
                    }
                    reg.insert(*dest, result_set);
                }
            },
            Instruction::Toggle { dest, op } => {
                let set = get_set_1(&reg, op)?;
                let title_set: HashSet<Title> = set.iter().cloned().map(|title| title.into_toggle_talk()).collect();
                reg.insert(*dest, title_set);
            },
            Instruction::Prefix { dest, op, cs } => {
                let set = get_set_1(&reg, op)?;
                if set.is_empty() {
                    reg.insert(*dest, HashSet::new());
                } else if set.len() > 1 {
                    return Err(SolveError::QueryForMultiplePages);
                } else {
                    let mut result_set: HashSet<Title> = HashSet::new();
                    for t in set.iter() {
                        let res_one = apisolver::get_prefix_index_one(t, cs.ns.as_ref(), cs.redir.unwrap_or(RedirectFilterStrategy::All), cs.limit.unwrap_or(default_limit)).await?;
                        result_set.extend(res_one);
                    }
                    reg.insert(*dest, result_set);
                }
            },
            Instruction::Set { dest, titles, cs } => {
                let mut title_set: HashSet<Title> = HashSet::new();
                for t in titles {
                    let title: Title = API_SERVICE.title_new_from_full(t)?;
                    if let Some(nss) = &cs.ns {
                        if !nss.contains(&title.namespace_id()) {
                            continue;
                        }
                    }
                    title_set.insert(title);
                }
                reg.insert(*dest, title_set);
            },
            Instruction::Nop { dest, op } => {
                let set = get_set_1(&reg, op)?;
                let copiedset = set.clone();
                reg.insert(*dest, copiedset);
            },
        }
    }

    let result = get_set_1(&reg, &query.1)?;
    Ok(result.clone())
}

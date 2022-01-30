//! This module runs validation and optimization
//! on an Abstract Syntax Tree (AST).
//! 

use std::collections::HashSet;

use plbot_base::ir::{Instruction, SetConstraint, RegID, DepthNum, RedirectFilterStrategy};
use plbot_base::NamespaceID;

use crate::{ast::*, error::PLBotParserError};

/// Convert a `Vec` of `Constraint`s into a `SetConstraint`
/// Merge all `Ns` constraints (using intersection), set all `Limit` constraints to the minimum, and reject any other duplicate-and-confilcting constraints
pub(crate) fn construct_constraints_from_vec(orig: &[Constraint]) -> Result<SetConstraint, PLBotParserError> {
    let mut depth: Option<DepthNum> = None;
    let mut ns: Option<HashSet<NamespaceID>> = None;
    let mut redir: Option<RedirectFilterStrategy> = None;
    let mut directlink: Option<bool> = None;
    let mut resolveredir: Option<bool> = None;
    let mut limit: Option<i64> = None;

    for c in orig {
        match c {
            Constraint::Ns(n) => {
                if let Some(old_set) = ns {
                    let new_set = n.iter().copied().collect();
                    let intersect_set = old_set.intersection(&new_set).copied().collect();
                    ns = Some(intersect_set);
                } else {
                    ns = Some(n.iter().copied().collect());
                }
            },
            Constraint::Depth(d) => {
                if let Some(n) = depth {
                    if n != *d && (n >= 0 || *d >= 0) { // Disallow different depth constraints, except they are both negative
                        return Err(PLBotParserError::Semantic("conflict depth".to_string()));
                    }
                } else {
                    depth = Some(*d);
                }
            }
            Constraint::Redir(s) => {
                if let Some(ss) = redir {
                    if ss != *s {
                        return Err(PLBotParserError::Semantic("conflict redirect strategy".to_string()));
                    }
                } else {
                    redir = Some(*s);
                }
            },
            Constraint::DirectLink(s) => {
                if let Some(ss) = directlink {
                    if ss != *s {
                        return Err(PLBotParserError::Semantic("conflict direct link constraint".to_string()));
                    }
                } else {
                    directlink = Some(*s);
                }
            },
            Constraint::ResolveRedir(s) => {
                if let Some(ss) = resolveredir {
                    if ss != *s {
                        return Err(PLBotParserError::Semantic("conflict resolveredir constraint".to_string()));
                    }
                } else {
                    resolveredir = Some(*s);
                }
            },
            Constraint::Limit(l) => {
                if let Some(ll) = limit {
                    if ll < 0 {
                        limit = Some(*l);
                    } else {
                        limit = Some(i64::min(*l, ll));
                    }
                } else {
                    limit = Some(*l);
                }
            }
        }
    }
    Ok( SetConstraint { ns, depth, redir, directlink, resolveredir, limit } )
}

/// Merge two `SetConstraint`s into one
/// `Ns` will be merged by intersection, `Limit` will get the minimum number, for other constraints, return error if they conflict.
pub(crate) fn merge_constraints(orig: &SetConstraint, other: &SetConstraint) -> Result<SetConstraint, PLBotParserError> {
    let ns = if orig.ns.is_none() {
        other.ns.clone()
    } else if other.ns.is_none() {
        orig.ns.clone()
    } else {
        Some(orig.ns.as_ref().unwrap().intersection(other.ns.as_ref().unwrap()).copied().collect())
    };
    let depth = if orig.depth.is_none() {
        other.depth
    } else if other.depth.is_none() || (orig.depth.unwrap() == other.depth.unwrap()) || (orig.depth.unwrap() < 0 && other.depth.unwrap() < 0) {
        orig.depth
    } else {
        return Err(PLBotParserError::Semantic(String::from("conflict depth")));
    };
    let redir = if orig.redir.is_none() {
        other.redir
    } else if other.redir.is_none() || orig.redir.unwrap() == other.redir.unwrap() {
        orig.redir
    } else {
        return Err(PLBotParserError::Semantic(String::from("conflict redirect strategy")));
    };
    let directlink = if orig.directlink.is_none() {
        other.directlink
    } else if other.directlink.is_none() || orig.directlink.unwrap() == other.directlink.unwrap() {
        orig.directlink
    } else {
        return Err(PLBotParserError::Semantic(String::from("conflict directlink constraint")));
    };
    let resolveredir = if orig.resolveredir.is_none() {
        other.resolveredir
    } else if other.resolveredir.is_none() || orig.resolveredir.unwrap() == other.resolveredir.unwrap() {
        orig.resolveredir
    } else {
        return Err(PLBotParserError::Semantic(String::from("conflict resolveredir constraint")));
    };
    let limit = if orig.limit.is_none() || orig.limit.unwrap() < 0 {
        other.limit
    } else if other.limit.is_none() || other.limit.unwrap() < 0 {
        orig.limit
    } else {
        Some(i64::min(orig.limit.unwrap(), other.limit.unwrap()))
    };

    Ok(SetConstraint { ns, depth, redir, directlink, resolveredir, limit })
}

/// Removes consecutive `Toggle` instructions
pub(crate) fn remove_redundent_talk(ir: &mut Vec<Instruction>) {
    // iterate through every instruction
    // if we encounter a `Toggle { dest, op }`, check the corresponding instruction whose `dest` is the aforementioned `Toggle` instruction's op
    // if that instruction is also a `Toggle { dest2, op2 }` i.e. `dest2 == op`
    // change the two instructions into `Nop { dest, op }` instructions
    for idx in 0..ir.len() {
        if let Instruction::Toggle { dest, op } = ir[idx] {
            if let Ok(idx2) = ir.binary_search_by(|probe| probe.get_dest().cmp(&op)) {
                if let Instruction::Toggle { dest: dest2, op: op2 } = ir[idx2] {
                    // change instructions
                    let inst1 = Instruction::Nop { dest, op };
                    let inst2 = Instruction::Nop { dest: dest2, op: op2 };
                    ir[idx] = inst1;
                    ir[idx2] = inst2;
                }
            }
        }
    }
}

/// Removes instructions that are destined to yield an empty set
/// 
/// This function mainly tests if an instruction has a namespace constraint
/// that is empty, i.e. a namespace constraint that allows pages from no namespaces.
/// Such an constraint ensures that it will always have an empty result.
pub(crate) fn remove_empty_ns(ir: &mut Vec<Instruction>) {
    // iterate through every instruction
    // if we encounter an instruction that `instruct.ns_empty() == true`
    // the whole subtree where that instruction resides, should be nop
    // since leaf nodes are always `Set` instruction, that instruction
    // is replaced with an empty `Set` instruction
    for idx in 0..ir.len() {
        if ir[idx].ns_empty() {
            // replace the whole subtree with nop
            let mut stack: Vec<RegID> = Vec::new();
            stack.push(ir[idx].get_dest());
            while let Some(opdest) = stack.pop() {
                // search for the instruction with the specified `dest`
                if let Ok(idx) = ir.binary_search_by(|probe| probe.get_dest().cmp(&opdest)) {
                    match &mut ir[idx] {
                        Instruction::And { op1, op2, .. } |
                        Instruction::Or { op1, op2, .. } |
                        Instruction::Exclude { op1, op2, .. } |
                        Instruction::Xor { op1, op2, .. } => {
                            stack.push(*op2);
                            stack.push(*op1);
                        }
                        Instruction::Link { dest, op, .. } |
                        Instruction::LinkTo { dest, op, .. } |
                        Instruction::EmbeddedIn { dest, op, .. } |
                        Instruction::InCat { dest, op, .. } |
                        Instruction::Toggle { dest, op } |
                        Instruction::Prefix { dest, op, .. } => {
                            let emptyinst = Instruction::Nop { dest: *dest, op: *op };
                            stack.push(*op);
                            ir[idx] = emptyinst;
                        },
                        Instruction::Set { dest: _, titles, cs } => {
                            titles.clear();
                            *cs = SetConstraint::new();
                        },
                        Instruction::Nop { dest: _, op } => {
                            stack.push(*op);
                        },
                    }
                }
            }
        }
    }
}

/// Removes all Nop instructions
pub(crate) fn remove_nop(ir: &mut Vec<Instruction>) {
    // iterate through every instruction
    let mut idx = 0;
    while idx < ir.len() {
        let mut deleted = false;
        if let Instruction::Nop { dest, op } = ir[idx] {
            while let Ok(idx2) = ir.binary_search_by(|probe| probe.get_dest().cmp(&op)) {
                ir[idx2].set_dest(dest);
                ir.remove(idx);
                deleted = true;
            }
        }
        if !deleted {
            idx += 1;
        }
    }
}

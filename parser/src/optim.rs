//! This module runs validation and optimization
//! on an Abstract Syntax Tree (AST).
//! 

use std::collections::HashSet;

use plbot_base::ir::{Instruction, SetConstraint, RegID, DepthNum, RedirectStrategy};
use plbot_base::NamespaceID;

use crate::{ast::*, error::SemanticError};

/// Convert a `Vec` of `Constraint`s into a `SetConstraint`
/// Merge all `Ns` constraints (using intersection), and reject any other duplicate-and-confilcting constraints
pub(crate) fn construct_constraints_from_vec(orig: &Vec<Constraint>) -> Result<SetConstraint, SemanticError> {
    let mut con_dep: Option<DepthNum> = None;
    let mut con_ns_set: Option<HashSet<NamespaceID>> = None;
    let mut con_redir: Option<RedirectStrategy> = None;
    let mut con_directlink: Option<bool> = None;

    for c in orig {
        match &*c {
            Constraint::Ns(n) => {
                if con_ns_set.is_none() {
                    con_ns_set = Some(n.into_iter().copied().collect());
                } else {
                    let old_set = con_ns_set.unwrap();
                    let new_set = n.into_iter().copied().collect();
                    let intersect_set = old_set.intersection(&new_set).copied().collect();
                    con_ns_set = Some(intersect_set);
                }
            },
            Constraint::Depth(d) => {
                if con_dep.is_none() {
                    con_dep = Some(*d);
                } else {
                    let n = con_dep.unwrap();
                    if n != *d && (n >= 0 || *d >= 0) { // Disallow different depth constraints, except they are both negative
                        return Err(SemanticError{ msg: "conflict depth".to_string() });
                    }
                }
            }
            Constraint::Redir(s) => {
                if con_redir.is_none() {
                    con_redir = Some(*s);
                } else {
                    let ss = con_redir.unwrap();
                    if ss != *s {
                        return Err(SemanticError{ msg: "conflict redirect strategy".to_string() });
                    }
                }
            },
            Constraint::DirectLink(s) => {
                if con_directlink.is_none() {
                    con_directlink = Some(*s);
                } else {
                    let ss = con_directlink.unwrap();
                    if ss != *s {
                        return Err(SemanticError{ msg: "conflict direct link constraint".to_string() });
                    }
                }
            },
        }
    }
    Ok( SetConstraint { ns: con_ns_set, depth: con_dep, redir: con_redir, directlink: con_directlink } )
}

/// Merge two `SetConstraint`s into one
/// `Ns` will be merged by intersection, for other constraints, return error if they conflict.
pub(crate) fn merge_constraints(orig: &SetConstraint, other: &SetConstraint) -> Result<SetConstraint, SemanticError> {
    let merged_ns = if orig.ns.is_none() {
        other.ns.clone()
    } else if other.ns.is_none() {
        orig.ns.clone()
    } else {
        Some(orig.ns.as_ref().unwrap().intersection(&other.ns.as_ref().unwrap()).copied().collect())
    };
    let merged_depth = if orig.depth.is_none() {
        other.depth
    } else if other.depth.is_none() {
        orig.depth
    } else if (orig.depth.unwrap() == other.depth.unwrap()) || (orig.depth.unwrap() < 0 && other.depth.unwrap() < 0) {
        orig.depth
    } else {
        return Err(SemanticError { msg: String::from("conflict depth") });
    };
    let merged_redir = if orig.redir.is_none() {
        other.redir
    } else if other.redir.is_none() {
        orig.redir
    } else if orig.redir.unwrap() == other.redir.unwrap() {
        orig.redir
    } else {
        return Err(SemanticError { msg: String::from("conflict redirect strategy") });
    };
    let merged_directlink = if orig.directlink.is_none() {
        other.directlink
    } else if other.directlink.is_none() {
        orig.directlink
    } else if orig.directlink.unwrap() == other.directlink.unwrap() {
        orig.directlink
    } else {
        return Err(SemanticError { msg: String::from("conflict directlink constraint") });
    };

    Ok(SetConstraint { ns: merged_ns, depth: merged_depth, redir: merged_redir, directlink: merged_directlink })
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
                        Instruction::LinkTo { dest, op, .. } |
                        Instruction::InCat { dest, op, .. } |
                        Instruction::Toggle { dest, op } |
                        Instruction::Prefix { dest, op, .. } => {
                            let emptyinst = Instruction::Nop { dest: *dest, op: *op };
                            stack.push(*op);
                            ir[idx] = emptyinst;
                        },
                        Instruction::Set { dest: _, titles, cs } => {
                            titles.clear();
                            *cs = SetConstraint { ns: None, depth: None, redir: None, directlink: None };
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

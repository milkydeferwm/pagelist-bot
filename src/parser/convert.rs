//! This module converts the optimized AST
//! into generic Intermediate Representation (IR)
//! defined in `plbot_base`

use std::collections::HashSet;

use super::{ast::Expr, ast::UnaryOpcode, ast::BinaryOpcode, PLBotParseResult, optim::merge_constraints, optim::construct_constraints_from_vec, error::PLBotParserError};
use super::ir::{Instruction, SetConstraint, RegID, RedirectFilterStrategy};

pub(crate) fn to_ir(ast: &Expr) -> PLBotParseResult {
    ir_helper(ast, 0)
}

fn ir_helper(ast: &Expr, mut reg_id: RegID) -> PLBotParseResult {
    // do a postorder dfs to the tree
    // find any semantic error
    let mut stack: Vec<&Expr> = Vec::new();
    let mut root = Some(ast);
    let mut inst: Vec<Instruction> = Vec::new();

    while let Some(node) = root {
        stack.push(node);
        match &node {
            Expr::Binary(..) => root = None,
            Expr::Unary(_, c) => root = Some(c),
            Expr::Constrained(c, _) => root = Some(c),
            Expr::Page(..) => root = None,
        };
    }

    while !stack.is_empty() {
        let node = stack.pop().unwrap();
        let instruct: Instruction;
        match &node {
            Expr::Page(l) => {
                instruct = Instruction::Set{ dest:reg_id, titles: l.to_owned(), cs: SetConstraint::new() };
                inst.push(instruct);
                reg_id += 1;
            },
            Expr::Unary(op, _) => {
                instruct = match *op {
                    UnaryOpcode::Link => Instruction::Link{ dest: reg_id, op: reg_id - 1, cs: SetConstraint::new() },
                    UnaryOpcode::LinkTo => Instruction::LinkTo{ dest: reg_id, op: reg_id - 1, cs: SetConstraint::new() },
                    UnaryOpcode::EmbeddedIn => Instruction::EmbeddedIn{ dest: reg_id, op: reg_id - 1, cs: SetConstraint::new() },
                    UnaryOpcode::InCategory => Instruction::InCat{ dest: reg_id, op: reg_id - 1, cs: SetConstraint::new() },
                    UnaryOpcode::Toggle => Instruction::Toggle{ dest: reg_id, op: reg_id - 1 },
                    UnaryOpcode::Prefix => Instruction::Prefix{ dest: reg_id, op: reg_id - 1, cs: SetConstraint::new() },
                };
                inst.push(instruct);
                reg_id += 1;
            },
            Expr::Binary(l, op, r) => {
                let mut lop = ir_helper(l, reg_id)?;
                let left_dest = lop.1;
                reg_id = left_dest + 1;
                inst.append(&mut lop.0);
                
                let mut rop = ir_helper(r, reg_id)?;
                let right_dest = rop.1;
                reg_id = right_dest + 1;
                inst.append(&mut rop.0);

                instruct = match *op {
                    BinaryOpcode::And => Instruction::And{ dest: reg_id, op1: left_dest, op2: right_dest },
                    BinaryOpcode::Or => Instruction::Or{ dest: reg_id, op1: left_dest, op2: right_dest },
                    BinaryOpcode::Exclude => Instruction::Exclude{ dest: reg_id, op1: left_dest, op2: right_dest },
                    BinaryOpcode::Xor => Instruction::Xor{ dest: reg_id, op1: left_dest, op2: right_dest },
                };
                inst.push(instruct);
                reg_id += 1;
            },
            Expr::Constrained(_, c) => {
                // apply the constraint to the corresponding instruction
                // the tree formulation ensures that this would always be the last element of `inst`, aka `reg_id - 1`
                // the instruction construction process ensures that `inst` is sorted by `dest` field in ascending order
                let constraint_struct = construct_constraints_from_vec(c)?;
                // rejects if ns has some negative number
                let mut stack: Vec<(RegID, SetConstraint)> = vec![(reg_id - 1, constraint_struct)];
                while let Some((target, con)) = stack.pop() {
                    let ires = inst.binary_search_by(|probe| probe.get_dest().cmp(&target));
                    if let Ok(idx) = ires {
                        match &mut inst[idx] {
                            Instruction::And { dest: _, op1, op2 } |
                            Instruction::Or { dest: _, op1, op2 } |
                            Instruction::Exclude { dest: _, op1, op2 } |
                            Instruction::Xor { dest: _, op1, op2 } => {
                                // If that instruction is a binary set operation, send the constraint into both branches
                                stack.push((*op2, con.clone()));
                                stack.push((*op1, con.clone()));
                            },
                            Instruction::Link { dest, op, cs } => {
                                // rejects if constraint has a depth or directlink field, else merge
                                if con.depth.is_some() || con.directlink.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid constraint")));
                                }
                                // also rejects if constraint has a redirect constraint other than `All`
                                if con.redir.is_some() && con.redir.unwrap() != RedirectFilterStrategy::All {
                                    return Err(PLBotParserError::Semantic(String::from("invalid redirect strategy")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::Link { dest: *dest, op: *op, cs: new_constraint };
                                inst[idx] = new_inst;
                            },
                            Instruction::LinkTo { dest, op, cs } => {
                                // rejects if constraint has a depth field, else merge
                                if con.depth.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid depth constraint")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::LinkTo { dest: *dest, op: *op, cs: new_constraint };
                                inst[idx] = new_inst;
                            },
                            Instruction::EmbeddedIn { dest, op, cs } => {
                                // rejects if constraint has a depth or directlink field, else merge
                                if con.depth.is_some() || con.directlink.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid constraint")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::EmbeddedIn { dest: *dest, op: *op, cs: new_constraint };
                                inst[idx] = new_inst;
                            }
                            Instruction::InCat { dest, op, cs } => {
                                // rejects if constraint has a redirect constraint other than `All`, or constraint has a directlink constraint. Otherwise merge the constraints
                                if con.redir.is_some() && con.redir.unwrap() != RedirectFilterStrategy::All {
                                    return Err(PLBotParserError::Semantic(String::from("invalid redirect strategy")));
                                }
                                if con.directlink.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid directlink constraint")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::InCat { dest: *dest, op: *op, cs: new_constraint };
                                inst[idx] = new_inst;
                            }
                            Instruction::Toggle { dest: _, op } => {
                                // switch every ns constraint, then pass through this instruction
                                let ns = con.ns.clone();
                                
                                if let Some(ns_set) = ns {
                                    let mut ns_vec = Vec::from_iter(ns_set);
                                    for i in ns_vec.iter_mut() {
                                        *i ^= 0b1;
                                    }
                                    let new_con = SetConstraint { ns: Some(HashSet::from_iter(ns_vec.into_iter())), depth: con.depth, redir: con.redir, directlink: con.directlink, resolveredir: con.resolveredir, limit: con.limit };
                                    stack.push((*op, new_con));
                                } else {
                                    stack.push((*op, con.clone()));
                                }
                            }
                            Instruction::Prefix { dest, op, cs } => {
                                // rejects if constraint has a depth, resolveredir, or directlink field
                                // else merge
                                if con.depth.is_some() || con.directlink.is_some() || con.resolveredir.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid constraint")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::Prefix { dest: *dest, op: *op, cs: new_constraint };
                                inst[idx] = new_inst;
                            },
                            Instruction::Nop { dest: _, op } => {
                                // pass through this instruction
                                stack.push((*op, con.clone()));
                            }
                            Instruction::Set { dest, titles, cs } => {
                                // rejects if constraint has a depth, redir, resolveredir, or directlink field, else merge
                                if con.depth.is_some() || con.redir.is_some() || con.directlink.is_some() || con.resolveredir.is_some() {
                                    return Err(PLBotParserError::Semantic(String::from("invalid constraint")));
                                }
                                let new_constraint = merge_constraints(cs, &con)?;
                                let new_inst = Instruction::Set { dest: *dest, titles: (*titles).clone(), cs: new_constraint };
                                inst[idx] = new_inst;
                            },
                        }
                    } else {
                        return Err(PLBotParserError::Semantic(String::from("internal instruction not found while generating")));
                    }
                }
            }
        }
    }

    Ok((inst, reg_id - 1))
}

//! Module containing the formal definition of the query.
//! 
//! This is different from `ast.rs` from `plbot_parser`,
//! because we can define different query syntax, 
//! but all of them should be converted into the query syntax
//! defined here.
//! 
//! Just like the intermediate representation (IR) in a compiler.

use crate::NamespaceID;
use std::collections::HashSet;

pub type RegID = u64;
pub type DepthNum = i32;

#[derive(Debug, Clone)]
pub struct SetConstraint {
    pub ns: Option<HashSet<NamespaceID>>,
    pub depth: Option<DepthNum>,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    // Binary
    And { dest: RegID, op1: RegID, op2: RegID },
    Or { dest: RegID, op1: RegID, op2: RegID },
    Exclude { dest: RegID, op1: RegID, op2: RegID },
    Xor { dest: RegID, op1: RegID, op2: RegID },
    // Unary
    LinkTo { dest: RegID, op: RegID, cs: SetConstraint },
    InCat { dest: RegID, op: RegID, cs: SetConstraint },
    Toggle { dest: RegID, op: RegID },
    Prefix { dest: RegID, op: RegID },
    // Primitive
    Set { dest: RegID, titles: Vec<String>, cs: SetConstraint },
    // Null
    Nop { dest: RegID, op: RegID },
}

impl Instruction {

    pub fn is_binary_op(&self) -> bool {
        match *self {
            Self::And {..} | Self::Or {..} | Self::Exclude {..} | Self::Xor {..} => true,
            _ => false,
        }
    }

    pub fn is_unary_op(&self) -> bool {
        match *self {
            Self::LinkTo {..} | Self::InCat {..} | Self::Toggle {..} | Self::Prefix {..} => true,
            _ => false,
        }
    }

    pub fn is_primitive_op(&self) -> bool {
        match *self {
            Self::Set {..} => true,
            _ => false,
        }
    }

    pub fn is_nop(&self) -> bool {
        match *self {
            Self::Nop {..} => true,
            _ => false,
        }
    }

    pub fn get_dest(&self) -> RegID {
        match *self {
            Self::And { dest, .. } => dest,
            Self::Or { dest, .. } => dest,
            Self::Exclude { dest, .. } => dest,
            Self::Xor { dest, .. } => dest,
            Self::LinkTo { dest, .. } => dest,
            Self::InCat { dest, .. } => dest,
            Self::Toggle { dest, ..} => dest,
            Self::Prefix { dest, .. } => dest,
            Self::Set { dest, .. } => dest,
            Self::Nop { dest, .. } => dest,
        }
    }

    pub fn set_dest(&mut self, new_dest: RegID) {
        match self {
            Self::And { dest, .. } => *dest = new_dest,
            Self::Or { dest, .. } => *dest = new_dest,
            Self::Exclude { dest, .. } => *dest = new_dest,
            Self::Xor { dest, .. } => *dest = new_dest,
            Self::LinkTo { dest, .. } => *dest = new_dest,
            Self::InCat { dest, .. } => *dest = new_dest,
            Self::Toggle { dest, ..} => *dest = new_dest,
            Self::Prefix { dest, .. } => *dest = new_dest,
            Self::Set { dest, .. } => *dest = new_dest,
            Self::Nop { dest, .. } => *dest = new_dest,
        };
    }

    pub fn ns_empty(&self) -> bool {
        match self {
            Self::LinkTo { cs, .. } |
            Self::InCat { cs, .. } |
            Self::Set { cs, .. } => {
                if let Some(ns) = &cs.ns {
                    ns.is_empty()
                } else {
                    false
                }
            },
            _ => false,
        }
    }

}
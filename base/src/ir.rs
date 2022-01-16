//! Module containing the formal definition of the query.
//! 
//! This is different from `ast.rs` from `plbot_parser`,
//! because we can define different query syntax, 
//! but all of them should be converted into the query syntax
//! defined here.
//! 
//! Just like the intermediate representation (IR) in a compiler.

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SetConstraint {
    pub ns: Option<HashSet<i32>>,
    pub depth: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    // Binary
    And { dest: i32, op1: i32, op2: i32 },
    Or { dest: i32, op1: i32, op2: i32 },
    Exclude { dest: i32, op1: i32, op2: i32 },
    Xor { dest: i32, op1: i32, op2: i32 },
    // Unary
    LinkTo { dest: i32, op: i32, cs: SetConstraint },
    InCat { dest: i32, op: i32, cs: SetConstraint },
    Toggle { dest: i32, op: i32 },
    Prefix { dest: i32, op: i32 },
    // Primitive
    Set { dest: i32, titles: Vec<String>, cs: SetConstraint },
    // Null
    Nop { dest: i32, op: i32 },
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

    pub fn get_dest(&self) -> i32 {
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

    pub fn set_dest(&mut self, new_dest: i32) {
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
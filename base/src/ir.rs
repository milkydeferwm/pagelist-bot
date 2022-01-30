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
pub type DepthNum = i64;

/// `RedirectFilterStrategy` controls whether the query result should include redirect pages.
/// Intended for `LinkTo` and `EmbeddedIn` instructions.
/// 
/// `NoRedirect`: filter out all redirect pages.
/// 
/// `OnlyRedirect`: explicitly query for redirects.
/// 
/// `All`: query for both redirects and non-redirects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectFilterStrategy {
    NoRedirect,
    OnlyRedirect,
    All,
}

impl ToString for RedirectFilterStrategy {
    fn to_string(&self) -> String {
        match self {
            Self::NoRedirect => String::from("nonredirects"),
            Self::OnlyRedirect => String::from("redirects"),
            Self::All => String::from("all"),
        }
    }
}

/// `SetConstraint` are modifier to some instructions.
/// They are intended for `Link`, `LinkTo`, `InCat`, `Prefix`, `EmbeddedIn` and `Set` instructions.
/// They are not effective to `Toggle` and and all binary instructions.
/// 
/// `ns`: the namespace(s) to filter on
/// 
/// `depth`: query depth into the category tree. Only to be used with `InCat`.
/// 
/// `redir`: how to deal with redirect pages. Refer to `RedirectStrategy` for more information. Only to be used with `LinkTo`, `Prefix` and `EmbeddedIn`.
/// 
/// `directlink`: how to deal with linking via redirects. Only to be used with `LinkTo`.
/// 
/// `resolveredir`: If a page is a redirect, how to deal with it.
#[derive(Debug, Clone)]
pub struct SetConstraint {
    pub ns: Option<HashSet<NamespaceID>>,
    pub depth: Option<DepthNum>,
    pub redir: Option<RedirectFilterStrategy>,
    pub directlink: Option<bool>,
    pub resolveredir: Option<bool>,
    pub limit: Option<i64>,
}

impl SetConstraint {
    pub fn new() -> Self {
        Self {
            ns: None,
            depth: None,
            redir: None,
            directlink: None,
            resolveredir: None,
            limit: None,
        }
    }
}

impl Default for SetConstraint {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum Instruction {
    // Binary
    And { dest: RegID, op1: RegID, op2: RegID },
    Or { dest: RegID, op1: RegID, op2: RegID },
    Exclude { dest: RegID, op1: RegID, op2: RegID },
    Xor { dest: RegID, op1: RegID, op2: RegID },
    // Unary
    Link { dest: RegID, op: RegID, cs: SetConstraint },
    LinkTo { dest: RegID, op: RegID, cs: SetConstraint },
    EmbeddedIn { dest: RegID, op: RegID, cs: SetConstraint },
    InCat { dest: RegID, op: RegID, cs: SetConstraint },
    Toggle { dest: RegID, op: RegID },
    Prefix { dest: RegID, op: RegID, cs: SetConstraint },
    // Primitive
    Set { dest: RegID, titles: Vec<String>, cs: SetConstraint },
    // Null
    Nop { dest: RegID, op: RegID },
}

impl Instruction {

    pub fn is_binary_op(&self) -> bool {
        matches!(*self, Self::And {..} | Self::Or {..} | Self::Exclude {..} | Self::Xor {..})
    }

    pub fn is_unary_op(&self) -> bool {
        matches!(*self, Self::Link {..} | Self::LinkTo {..} | Self::EmbeddedIn {..} | Self::InCat {..} | Self::Toggle {..} | Self::Prefix {..})
    }

    pub fn is_primitive_op(&self) -> bool {
        matches!(*self, Self::Set {..})
    }

    pub fn is_nop(&self) -> bool {
        matches!(*self, Self::Nop {..})
    }

    pub fn get_dest(&self) -> RegID {
        match *self {
            Self::And { dest, .. } => dest,
            Self::Or { dest, .. } => dest,
            Self::Exclude { dest, .. } => dest,
            Self::Xor { dest, .. } => dest,
            Self::Link { dest, .. } => dest,
            Self::LinkTo { dest, .. } => dest,
            Self::EmbeddedIn { dest, .. } => dest,
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
            Self::Link { dest, .. } => *dest = new_dest,
            Self::LinkTo { dest, .. } => *dest = new_dest,
            Self::EmbeddedIn { dest, .. } => *dest = new_dest,
            Self::InCat { dest, .. } => *dest = new_dest,
            Self::Toggle { dest, ..} => *dest = new_dest,
            Self::Prefix { dest, .. } => *dest = new_dest,
            Self::Set { dest, .. } => *dest = new_dest,
            Self::Nop { dest, .. } => *dest = new_dest,
        };
    }

    pub fn ns_empty(&self) -> bool {
        match self {
            Self::Link { cs, .. } |
            Self::LinkTo { cs, .. } |
            Self::EmbeddedIn { cs, .. } |
            Self::InCat { cs, .. } |
            Self::Prefix { cs, .. } |
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
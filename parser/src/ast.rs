//! This file lists the data structures used in
//! abstract syntax tree (AST) building.

use plbot_base::{NamespaceID, ir::{DepthNum, RedirectStrategy}};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Expr {
    // The ultimate primitive
    Page(Vec<String>),
    // Generative functions
    Unary(UnaryOpcode, Box<Expr>),
    // Constrained
    Constrained(Box<Expr>, Vec<Constraint>),
    // Set arithmetics
    Binary(Box<Expr>, BinaryOpcode, Box<Expr>),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum UnaryOpcode {
    LinkTo,
    InCategory,
    Toggle,
    Prefix,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BinaryOpcode {
    And,
    Or,
    Exclude,
    Xor,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum Constraint {
    Ns(Vec<NamespaceID>),
    Depth(DepthNum),
    Redir(RedirectStrategy),
    DirectLink(bool),
}

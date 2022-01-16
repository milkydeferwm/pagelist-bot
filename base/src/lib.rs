//! plbot_base
//! Base definitions across multiple crates
//! 

pub use mediawiki::api::NamespaceID;

pub mod ir;

pub type Query = (Vec<ir::Instruction>, ir::RegID);

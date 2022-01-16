//! plbot_base
//! Base definitions across multiple crates
//! 

pub mod ir;

pub type Query = (Vec<ir::Instruction>, i32);

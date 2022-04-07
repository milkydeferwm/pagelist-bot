//! Page List Bot actual workflow and routines
//! 

pub mod daemon;
pub mod task;
mod types;
mod output;
mod pagewriter;

pub use daemon::task_daemon;
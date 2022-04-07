//! Page List Bot actual workflow and routines
//! 

pub mod daemon;
pub mod task;
mod types;
mod output;
mod pagewriter;
mod queryexecutor;
mod taskrunner;

pub use daemon::task_daemon;
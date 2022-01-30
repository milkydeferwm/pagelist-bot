//! Page List Bot actual workflow and routines
//! 

pub mod daemon;
pub mod task;
mod types;
mod output;

pub use daemon::task_daemon;
pub use types::{LoginCredential, SiteProfile};
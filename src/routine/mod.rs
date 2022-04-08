//! Page List Bot actual workflow and routines
//! 

// pub mod daemon;
// pub mod task;
// mod output;
pub mod taskfinder;
pub mod taskrunner;
mod queryexecutor;
mod pagewriter;

mod types;

// pub use daemon::task_daemon;
pub use taskfinder::TaskFinder;
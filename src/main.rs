extern crate mediawiki;
extern crate clap;
extern crate tokio;
extern crate serde_json;
extern crate plbot_base;
extern crate plbot_parser;
extern crate plbot_solver;

use serde_json::Value;
use std::fs;

mod output;
mod routine;
mod arg;

/// The main function parses command line arguments, and extracts important information from config files.
/// Anything related to API is then spawned to `task_daemon`.
#[tokio::main]
async fn main() {
    let args = arg::build_argparse().get_matches();
    let sites = fs::read_to_string(args.value_of("site").unwrap()).expect("cannot open site information file");
    let sites: Value = serde_json::from_str(&sites).expect("cannot parse site information file");
    let profile = args.value_of("profile").unwrap();
    let site_prof: plbot_base::bot::SiteProfile = serde_json::from_value(sites[profile].clone()).expect("cannot find specified site profile");
    let login = fs::read_to_string(args.value_of("login").unwrap()).expect("cannot open login file");
    let login: Value = serde_json::from_str(&login).expect("cannot parse login file.");
    let login: plbot_base::bot::LoginCredential = serde_json::from_value(login[&site_prof.login].clone()).expect("cannot find specified site profile");

    let _daemon_handler = tokio::spawn(
        routine::task_daemon(login, site_prof)
    );

    match tokio::signal::ctrl_c().await {
        Ok(()) => {},
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        },
    }

    println!("Shut down all tasks.");
}

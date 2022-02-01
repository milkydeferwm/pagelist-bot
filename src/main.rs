extern crate mediawiki;
extern crate clap;
extern crate tokio;
extern crate tracing;
extern crate tracing_subscriber;
extern crate serde_json;
extern crate plbot_base;
extern crate plbot_parser;
extern crate plbot_solver;

use std::fs;
use serde_json::Value;
use mediawiki::api::Api;
use tracing::{info_span, debug, info, error, Level, Instrument};
use tracing_subscriber::fmt::format::FmtSpan;

mod routine;
mod arg;

/// The main function parses command line arguments, and extracts important information from config files.
/// Anything related to API is then spawned to `task_daemon`.
#[tokio::main]
async fn main() {
    // set up subscriber
    #[cfg(debug_assertions)]
    tracing_subscriber::fmt().with_max_level(Level::DEBUG).with_span_events(FmtSpan::CLOSE).init();
    #[cfg(not(debug_assertions))]
    tracing_subscriber::fmt().with_max_level(Level::INFO).with_span_events(FmtSpan::NONE).init();

    let args = info_span!(target: "bootstrap", "cli arg").in_scope(|| {
        debug!(target: "bootstrap", "parsing command line arguments");
        arg::build_argparse().get_matches()
    });

    let (profile, login) = info_span!(target: "bootstrap", "local config").in_scope(|| {
        info!(target: "bootstrap", "reading config files");
        debug!(target: "bootstrap", "reading site information file");
        let sites = fs::read_to_string(args.value_of("site").unwrap()).expect("cannot open site information file");
        debug!(target: "bootstrap", "parsing site information file");
        let sites: Value = serde_json::from_str(&sites).expect("cannot parse site information file");

        let profile = args.value_of("profile").unwrap();
        debug!(target: "bootstrap", "fetching profile \"{}\"", profile);
        let profile: routine::SiteProfile = serde_json::from_value(sites[profile].clone()).expect("cannot find specified site profile");

        debug!(target: "bootstrap", "reading login file");
        let login = fs::read_to_string(args.value_of("login").unwrap()).expect("cannot open login file");
        debug!(target: "bootstrap", "parsing login file");
        let login: Value = serde_json::from_str(&login).expect("cannot parse login file.");
        debug!(target: "bootstrap", "fetching login credential \"{}\"", &profile.login);
        let login: routine::LoginCredential = serde_json::from_value(login[&profile.login].clone()).expect("cannot find specified site profile");

        info!(target: "bootstrap", "read config files success");
        (profile, login)
    });

    // initialize mediawiki api instance
    let mut api = async {
        info!(target: "bootstrap", "creating API object");
        debug!(target: "bootstrap", "accessing MediaWiki Action API endpoint \"{}\"", &profile.api);
        let mut api: Api = Api::new(&profile.api).await.expect("cannot access target MediaWiki instance");
        debug!(target: "bootstrap", "setting up API object maxlag");
        api.set_maxlag(Some(5));
        debug!(target: "bootstrap", "setting up API max retry attempts");
        api.set_max_retry_attempts(3);
        debug!(target: "bootstrap", "setting up API user agent");
        api.set_user_agent(format!("Page List Bot / via User:{}", &login.username));
        debug!(target: "bootstrap", "API user agent: {}", api.user_agent());
        info!(target: "bootstrap", "creating API object success");
        api
    }.instrument(info_span!(target: "bootstrap", "api init")).await;

    async {
        info!(target: "bootstrap", "logging in as user \"{}\"", &login.username);
        api.login(&login.username, &login.password).await.expect("cannot log in");
        info!(target: "bootstrap", "logging in as user \"{}\" success", &login.username);
    }.instrument(info_span!(target: "bootstrap", "log in")).await;

    async {
        info!(target: "bootstrap", "starting up task daemon");
        tokio::select! {
            _ = routine::task_daemon(profile.config.clone(), api.clone(), profile.assert) => {
                error!(target: "bootstrap", "task daemon unexpectedly exits");
            }
            ctrl_c_res = tokio::signal::ctrl_c() => {
                match ctrl_c_res {
                    Ok(()) => { info!(target: "bootstrap", "ctrl-c detected") },
                    Err(err) => {
                        error!(target: "bootstrap", "unable to listen for shutdown signal: {}", err);
                    },
                }
            }
        };
    }.instrument(info_span!(target: "bootstrap", "main")).await;

    info_span!(target: "bootstrap", "clean up").in_scope(|| info!(target: "bootstrap", "shut down all tasks"));
}

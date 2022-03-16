use clap::{Command, Arg, crate_version};

pub fn build_argparse() -> Command<'static> {
    Command::new("Page List Bot")
        .about("Generate a list of wiki pages based on numerous criteria and set operations")
        .version(crate_version!())
        .args(&[
            Arg::new("login")
                .long("login")
                .required(true)
                .takes_value(true)
                .help("Path to the JSON file with username and password"),
            Arg::new("site")
                .long("site")
                .required(true)
                .takes_value(true)
                .help("Path to the JSON file with the website's information"),
            Arg::new("profile")
                .long("profile")
                .required(true)
                .takes_value(true)
                .help("The specific site profile in site information file to use")
        ])
}
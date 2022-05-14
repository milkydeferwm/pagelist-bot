# Page List Bot
[![Build Test](https://github.com/milkydeferwm/pagelist-bot/actions/workflows/test.yml/badge.svg)](https://github.com/milkydeferwm/pagelist-bot/actions/workflows/test.yml)

This is the source code for an automatic bot that generates a list of pages dedicated to MediaWiki.

**This repository is frozen, no bug fixes or feature additions will be made. A thorough revamp is ongoing at [milkydeferwm/pagelistbot](https://github.com/milkydeferwm/pagelistbot).**

## Usage
Command syntax:
```
pagelist-bot --site <SITES> --profile <PROFILE> --login <LOGIN>
```
All three arguments are mandatory.
You need two `json` files in order to run the bot. The details of these two files are described below.
### Site Profile
`--site <SITES>` refers to a `json` file which stores a list of site profiles. Each profile contains a list of the following items:
- `api`: The address of MediaWiki Action API for the target MediaWiki instance.
- `db` (Optional): **IN DEVELOPMENT** The database name for the target MediaWiki instance. You can omit this field if you cannot access the database.
- `assert` (Optional): Include this field if you want to use the assert module of MediaWiki Action API to ensure that you have the appropriate user right. Possible values: `anon`, `user`, `bot`.
- `login`: The login credential to use in the login file.
- `config`: The page name of the bot work configuration on-wiki.

Example (`example_profiles.json`):
```
{
    "enwiki": {
        "api": "https://en.wikipedia.org/w/api.php",
        "db": "enwiki_p",
        "login": "wikimedia"
        "assert": "bot",
        "config": "User:Example/config.json"
    },
    "meta": {
        "api": "https://meta.wikimedia.org/w/api.php",
        "login": "wikimedia",
        "config": "User:Example/config.json"
    }
}
```
This `json` file defines two profiles: `enwiki` and `meta`, which refers to [English Wikipedia](https://en.wikipedia.org) and [Wikimedia Meta-Wiki](https://meta.wikimedia.org) respectively. You can add other profiles (such as Fandom sites) too.

`--profile <PROFILE>` decides which profile should the bot use. The bot can work in English Wikipedia by setting `--profile enwiki`, or in Meta-Wiki by setting `--profile meta`, without writing a separate site configuration file.
### Login Credential
`--login <LOGIN>` refers to a `json` file which stores a list of login credentials for the bot. Each credential contains the following items:
- `username`: The username of the account.
- `password`: The password of that account.

Note: You cannot log in via your username and password. Instead, you should first go to `Special:BotPasswords` of the target website and generate a username and password for the bot to sign in.

Example (`example_credentials.json`):
```
{
    "wikimedia": {
        "username": "Example@bot",
        "password": "********"
    },
    "another": {
        "username": "Example@bot2",
        "password": "***********"
    }
}
```
This example `json` file provides the login credential `wikimedia` for the site profile shown above. If the bot cannot find the corresponding login credential required for that profile, the bot will panic and exit. You can add other login credentials to the same file for other site profiles.

### Run the Bot
To run the bot on English Wikipedia using the above two example `json` files, write your command as follows:
```
pagelist-bot --site /path/to/example_profiles.json --profile enwiki --login /path/to/example_credentials.json
```
You can run another bot for Meta-Wiki concurrently by
```
pagelist-bot --site /path/to/example_profiles.json --profile meta --login /path/to/example_credentials.json
```
Without creating a separate profile file and credential file.

## Build
The project is written in [Rust](https://www.rust-lang.org). To compile it, simply clone the repository and run
```
cargo build --release
```
This will build the project using the release profile (optimized). For an unoptimized build, drop `--release` in the build command. You will need the Rust toolchain.

## License and Attributions
This repository is available under MIT License. You may also be interested in
- [PetScan](https://github.com/magnusmanske/petscan_rs), which provides similar (and more powerful) functionality, also in Rust.
- [mediawiki_rust](https://github.com/magnusmanske/mediawiki_rust), the crate used in this project that interacts with MediaWiki Action API.

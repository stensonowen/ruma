//! Ruma is a Matrix homeserver client API.

#![feature(proc_macro, try_from)]
#![deny(missing_docs)]

extern crate argon2rs;
extern crate base64;
extern crate bodyparser;
extern crate chrono;
extern crate clap;
extern crate env_logger;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
#[macro_use] extern crate iron;
#[cfg(test)] extern crate iron_test;
#[macro_use] extern crate log;
#[macro_use] extern crate slog;
#[macro_use] extern crate slog_scope;
extern crate slog_term;
extern crate macaroons;
extern crate mount;
extern crate plugin;
extern crate persistent;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate rand;
extern crate router;
extern crate ruma_events;
extern crate ruma_identifiers;
extern crate rustc_serialize;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serde_yaml;
extern crate toml;
extern crate unicase;

use clap::{App, AppSettings, SubCommand, Arg};

use config::Config;
use crypto::generate_macaroon_secret_key;
use server::Server;

#[macro_use]
pub mod middleware;
pub mod access_token;
/// API endpoints as Iron handlers.
pub mod api {
    pub mod r0;
}
pub mod account_data;
pub mod authentication;
pub mod config;
pub mod crypto;
pub mod db;
pub mod error;
pub mod event;
pub mod modifier;
pub mod profile;
pub mod room;
pub mod room_alias;
pub mod schema;
pub mod server;
pub mod swagger;
pub mod room_membership;
#[cfg(test)] pub mod test;
pub mod user;

use slog::DrainExt;
embed_migrations!();

fn main() {
    env_logger::init().expect("Failed to initialize logger.");

	let drain = slog_term::streamer().compact().build().fuse();
    let log = slog::Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")));
	slog_scope::set_global_logger(log);

    info!("Initializing argument parsing");
    let matches = App::new("ruma")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A Matrix homeserver client API")
        .setting(AppSettings::GlobalVersion)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("run")
                .about("Runs the Ruma server")
                .arg(Arg::with_name("config")
                     .short("c")
                     .long("config")
                     .value_name("FILE")
                     .help("Define a custom config file (defaults to `ruma.[json|toml|yaml]`)")
                     .takes_value(true)
                     )
        )
        .subcommand(
            SubCommand::with_name("secret")
                .about("Generates a random value to be used as a macaroon secret key")
        )
        .get_matches();


    info!("Beginning to process argument parsing");
    match matches.subcommand() {
        ("run", Some(subcmd)) => {
            let config = match Config::from_file(subcmd.value_of("config")) {
                Ok(config) => config,
                Err(error) => {
                    info!("Either no file was found or it failed to open");
                    println!("Failed to load configuration file: {}", error);

                    return;
                }
            };

            match Server::new(&config) {
                Ok(server) => {
                    if let Err(error) = server.run() {
                        info!("Runtime error: {}", error);
                        println!("{}", error);
                    }
                },
                Err(error) => {
                    info!("Failed to create server: {}", error);
                    println!("Failed to create server: {}", error);

                    return;
                }
            }
        }
        ("secret", Some(_)) => match generate_macaroon_secret_key() {
            Ok(key) => {
                info!("Generating macaroon secret");
                println!("{}", key)
            },
            Err(error) => {
                info!("Failed to generate macaroon secret key: {}", error);
                println!("Failed to generate macaroon secret key: {}", error)
            },
        },
        _ => println!("{}", matches.usage()),
    };
}

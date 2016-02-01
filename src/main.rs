//! Ruma is a server for Matrix.org's client-server API.

#![feature(custom_attribute, custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate bodyparser;
extern crate clap;
#[macro_use] extern crate diesel;
extern crate env_logger;
extern crate hyper;
#[macro_use] extern crate iron;
#[macro_use] extern crate log;
extern crate mount;
extern crate persistent;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate rand;
extern crate router;
extern crate serde;
extern crate serde_json;

mod api {
    pub mod r0 {
        pub mod authentication;
        pub mod versions;
    }
}
mod config;
mod db;
mod error;
mod middleware;
mod modifier;
mod server;
mod tables;

use clap::{App, AppSettings, SubCommand};

use config::Config;
use server::Server;

fn main() {
    env_logger::init().expect("Failed to initialize logger.");

    let matches = App::new("ruma")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A Matrix homeserver")
        .setting(AppSettings::GlobalVersion)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("start")
                .about("Starts the Ruma server")
        )
        .get_matches();

    match matches.subcommand() {
        ("start", Some(_matches)) => {
            let config = match Config::load("ruma.json") {
                Ok(config) => config,
                Err(error) => {
                    println!("Failed to load configuration file: {}", error);

                    return;
                }
            };

            match Server::new(&config) {
                Ok(server) => {
                    if let Err(error) = server.start() {
                        println!("{}", error);
                    }
                },
                Err(error) => {
                    println!("Failed to create server: {}", error);

                    return;
                }
            }
        },
        _ => println!("{}", matches.usage()),
    };
}

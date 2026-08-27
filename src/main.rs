mod commands;
pub mod user;
pub mod prompt;
pub mod db;
pub mod template;

use std::process::exit;
use clap::{Command, arg, value_parser};
use addr::parse_domain_name;
use std::env;

fn main() {
    let matches = Command::new("DevMan")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            arg!(-v --verbose ... "Set the level of verbosity")
        )
        .arg(
            arg!(
                -n --new <DOMAIN> "Site domain"
            )
            .required(false)
            .value_parser(value_parser!(String)),
        )
        .arg(
            arg!(
                -r --remove <DOMAIN> "Domain for the site to be removed"
            )
            .required(false)
            .value_parser(value_parser!(String)),
        )
        .get_matches();

    let v = match matches
        .get_one::<u8>("verbose")
        .expect("Expected verbose flag")
    {
        // default
        0 => 0,
        // some verbosity
        1 => 1,
        // max verbosity
        2 => 2,
        // anything else
        _ => 2,
    };

    if let Some(domain) = matches.get_one::<String>("new") {
        let validation_result = parse_domain_name(&domain);

        if validation_result.is_err() {
            println!("{}", validation_result.unwrap_err());
            exit(1);
        }

        commands::new_site(domain, v);
    }

    if let Some(remove) = matches.get_one::<String>("remove") {
        println!("remove domain: {}", remove);
    }
}

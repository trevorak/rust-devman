mod commands;
pub mod user;
pub mod prompt;
pub mod db;
pub mod template;
mod http;

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
                -n --new <DOMAIN> "Configure a new site"
            )
            .required(false)
            .value_parser(value_parser!(String)),
        )
        .arg(
            arg!(
                -r --remove <DOMAIN> "Remove a site's configuration"
            )
            .required(false)
            .value_parser(value_parser!(String)),
        )
        // TODO: Add restart command
        //  devman restart site.domain (restarts php-{version}-fpm and apache)
        .arg(
            arg!(
                --restart <DOMAIN> "Restart Apache and the PHP-FPM service being used by a site"
            )
                .value_parser(value_parser!(String))
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

        match validation_result {
            Ok(_) => {
                commands::new_site(domain, v);
            },
            Err(e) => {
                eprintln!("{}", e);
                exit(1);
            }
        }
    }

    if let Some(domain) = matches.get_one::<String>("remove") {
        commands::remove_site(&domain, v)
    }

    if let Some(domain) = matches.get_one::<String>("restart") {
        commands::restart_site(&domain, v)
    }
}

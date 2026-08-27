use std::{env, fs};
use std::os::unix::fs as unix_fs;
use std::process::Command as ShellCommand;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use regex::Regex;
use crate::{db, prompt};
use crate::db::get_default_config;
use crate::template::get_apache_config_template;

pub fn new_site(domain: &String, verbose: u8) {
    // let username = env::var("USER").unwrap();

    sudo::with_env(&["USER", "RUST_BACKTRACE"]).unwrap();

    let site_path = prompt_for_directory_input(
        "Enter site path (if different from current path): "
    );

    // prompt for WP install
    let is_wp = prompt_for_wordpress_install();

    let mut doc_root = site_path.clone();

    // if not WP, prompt for doc root
    if !is_wp {
        doc_root = prompt_for_directory_input(
            "Enter document root (relative to the site path): "
        );
    }

    // get mysql user
    let db_user = prompt::get_string("Enter MySQL user: ");

    // get mysql pass
    let db_pass = prompt::get_string("Enter MySQL password: ");
    _ = db_pass;

    // get php version
    let php_version = prompt_for_php_version();

    let db_name = slugify(&domain);

    println!();
    println!("Site URL: //{}", domain);
    println!("Site path: {}", site_path);
    println!("Document root: {}", doc_root);
    println!("DB User: {}", db_user);
    println!("DB Name: {}", db_name);
    println!("PHP: {}", php_version);
    println!("WP: {}", is_wp);

    // println!("username: {}", username);

    println!();
    let confirm = prompt::get_string("Confirm (Y/n) [Y]:");
    if confirm.to_lowercase() == "n" {
        return;
    }

    // create database
    _ = db::create_database(db_name, get_default_config(&db_pass));

    // TODO: If is_wp install and configure wordpress

    // get the document root with symlink
    // it will be composed of the site domain + the path after the site_root in doc_root
    // e.g.
    // site: site.local
    // site_root /path/to/site
    // doc_root /path/to/site/public
    // apache_doc = site.local/public
    let mut link_doc_root = String::from(domain);
    if let Some((_, after)) = doc_root.split_once(&site_path) {
        link_doc_root.push_str(&after.to_string());
    }

    if verbose > 1 {
        println!("Symlink doc root: {}", link_doc_root);
    }

    let conf_path = format!("/etc/apache2/sites-available/{}.conf", domain);
    let enabled_conf_path = format!("/etc/apache2/sites-enabled/{}.conf", domain);

    if verbose >= 1 {
        println!("Enabled conf: {}", conf_path);
        println!("Enabled conf: {}", enabled_conf_path);
    }

    let apache_conf = get_apache_config_template(
        domain,
        &link_doc_root,
        &php_version
    );

    // write config file to config_path
    if Path::new(&conf_path).exists() {
        println!("Site configuration already exists: {}", conf_path);
    } else {
        if verbose >= 2 {
            println!("Creating site config: {}", conf_path)
        }

        fs::write(&conf_path, &apache_conf).unwrap();
    }

    if verbose >= 1 {
        println!("Creating Apache config: {}", conf_path);
    }

    if verbose >= 1 {
        println!("Creating symbolic link: {} -> {}", conf_path, enabled_conf_path);
    }

    // create site-enabled symlink
    unix_fs::symlink(&conf_path, &enabled_conf_path).unwrap();

    let www_path = String::from(format!("/var/www/{}", domain));
    if Path::new(&www_path).exists() {
        println!("Skipping symbolic link. Path exists: {}", www_path);
    } else {
        if verbose >= 2 {
            println!("Creating symbolic link: {}", www_path);
        }

        // symlink site path and /var/www/html/site.domain
        unix_fs::symlink(site_path, &www_path).unwrap();
    }

    if verbose >= 1 {
        println!("Mapping DNS...")
    }

    let dns_entry = format!("127.0.0.1 {}", domain);

    let dns_content = fs::read_to_string("/etc/hosts");
    if dns_content.unwrap().contains(&dns_entry) {
        if verbose >= 1 {
            print!("Hosts file already contains entry: {}", dns_entry);
        }
    } else {
        // append dnsEntry to hosts file
        let mut hosts_file = OpenOptions::new()
            .append(true)
            .open("/etc/hosts")
            .unwrap();

        hosts_file.write_all(dns_entry.as_bytes()).unwrap();

        if verbose >= 1 {
            print!("Hosts file updated: {}", dns_entry);
        }
    }

    // restart apache
    let output = ShellCommand::new("service")
        .arg("apache2")
        .arg("restart")
        .output()
        .unwrap();

    if !output.status.success() {
        println!("Failed to restart Apache service: {}", output.status);
    } else if verbose >= 1 {
        println!("Apache service restarted");
    }

    println!("Setup complete: {}", domain);
}

// replace with underscores
fn slugify(s: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9_-]").unwrap();

    re.replace_all(s, "-").into_owned()
}

fn prompt_for_php_version() -> String {
    loop {
        let php_version = prompt::get_string("Enter PHP version (8.5): ");

        if php_version.contains('.') && php_version.parse::<f64>().is_ok() {
            return php_version;
        } else {
            println!("Invalid PHP version.");
        }
    }
}

fn prompt_for_wordpress_install() -> bool {
    let input = prompt::get_string("Install WordPress? (Y/n) [n]: ");

    if input.to_lowercase() == "y" {
        return true;
    }

    false
}

// Prompts for a path in a loop, until either:
// The input is empty, so it returns the PWD
// A valid existing path is entered
// Tha absolute path with trailing slash trimmed is returned
fn prompt_for_directory_input(msg: &str) -> String {
    loop {
        let mut input = prompt::get_string(msg);

        let current_dir = env::current_dir().unwrap();

        // if input is empty, return PWD
        if input.is_empty() {
            if current_dir.is_dir() {
                return normalize_path(
                    current_dir.to_str().unwrap()
                ).to_string();
            }
        }

        if !input.starts_with("/") {
            let dir_str = current_dir.to_str().unwrap().to_string();
            input = format!("{}/{}", dir_str, input);
        }

        if Path::new(&input).exists() {
            return normalize_path(&input).to_string();
        } else {
            println!("{} does not exist", input);
        }
    }
}

// Trim any trailing slash from paths
fn normalize_path(path: &str) -> &str {
    path.trim_end_matches("/")
}
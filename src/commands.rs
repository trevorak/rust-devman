use std::{
    env,
    fs::{
        self,
        File,
        OpenOptions,
    },
    io::{
        self,
        Write,
    },
    os::unix::fs as unix_fs,
    path::Path,
    process::Command as ShellCommand,
};
use std::io::BufRead;
use flate2::read::GzDecoder;
use regex::Regex;
use tokio::runtime::Builder;
use tar::Archive;
use crate::{
    db,
    http,
    prompt,
    template,
};

pub fn new_site(domain: &String, verbose: u8) {
    let vprint = get_verbose_conditional_print(verbose);

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
    let confirm = prompt::get_string("Confirm (Y/n) [Y]: ");
    if confirm.to_lowercase() == "n" {
        return;
    }

    // create database
    match Builder::new_current_thread().enable_all().build() {
        Ok(rt) => {
            let db_result = rt.block_on(
                db::create_database(&db_name, db::get_default_config(&db_pass))
            );

            match db_result {
                Ok(_) => {
                    vprint(1, "Database created")
                },
                Err(err) => {
                    eprintln!("Database error: {}", err);
                }
            }
        },
        Err(e) => {
            eprintln!("Thread builder error: {}", e);
        }
    }

    if is_wp {
        _ = setup_wp(
            &site_path,
            &db_name,
            &db_user,
            &db_pass,
            verbose
        );
    }

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

    vprint(1, format!("Symlink doc root: {}", link_doc_root).as_str());

    let conf_path = format!("/etc/apache2/sites-available/{}.conf", domain);
    let enabled_conf_path = format!("/etc/apache2/sites-enabled/{}.conf", domain);

    vprint(1, format!("Conf path: {}", conf_path).as_str());
    vprint(1, format!("Enable conf path: {}", enabled_conf_path).as_str());

    let apache_conf = template::get_apache_config_template(
        domain,
        &link_doc_root,
        &php_version
    );

    // write config file to config_path
    if Path::new(&conf_path).exists() {
        println!("Site configuration already exists: {}", conf_path);
    } else {
        vprint(2, format!("Creating site config: {}", conf_path).as_str());

        let result = fs::write(&conf_path, &apache_conf);

        match result {
            Ok(_) => {
                vprint(1, "Site configuration created")
            }
            Err(err) => {
                eprintln!("Error writing site config: {}", err);
            }
        }
    }

    vprint(1, format!("Creating symbolic link: {} -> {}", conf_path, enabled_conf_path).as_str());

    // create site-enabled symlink
    if Path::new(&enabled_conf_path).exists() {
        println!("Site configuration already exists: {}", enabled_conf_path);
    } else {
        match unix_fs::symlink(&conf_path, &enabled_conf_path) {
            Ok(_) => {
                vprint(1, "Symlink created")
            },
            Err(err) => {
                eprintln!("Apache config symlink creation failed: {}", err);
            }
        }
    }

    let www_path = String::from(format!("/var/www/html/{}", domain));
    if Path::new(&www_path).exists() {
        println!("Skipping symbolic link. Path exists: {}", www_path);
    } else {
        vprint(2, format!("Creating symbolic link: {}", www_path).as_str());

        // symlink site path and /var/www/html/site.domain
        match unix_fs::symlink(site_path, &www_path) {
            Ok(_) => {
                vprint(1, "Web root symlink created")
            },
            Err(err) => {
                eprintln!("Web root symlink creation failed: {}", err);
            }
        }
    }

    vprint(1, "Mapping DNS...");

    let dns_entry = format!("127.0.0.1 {}", domain);

    let dns_content = fs::read_to_string("/etc/hosts");
    match dns_content {
        Ok(content) => {
           if content.contains(&dns_entry) {
               vprint(1, format!("Hosts file already contains entry: {}", dns_entry).as_str());
           } else {
               // append dnsEntry to hosts file
               let hosts_file_create = OpenOptions::new()
                   .append(true)
                   .open("/etc/hosts");

               match hosts_file_create {
                   Ok(mut file) => {
                       _ = file.write_all(dns_entry.as_bytes());

                       vprint(1, format!("Hosts file updated: {}", dns_entry).as_str());
                   },
                   Err(_) => {
                       println!("Failed to write to hosts file: {}", dns_entry);
                   }
               }
           }
        },
        Err(err) => {
            eprintln!("Failed reading /etc/hosts: {}", err);
        }
    }

    // restart apache
    let result = restart_apache();
    if let Err(e) = result {
        eprintln!("{}", e);
    } else {
        vprint(1, "Apache restarted successfully")
    }

    println!("Setup complete: {}", domain);
}

pub fn remove_site(domain: &str, verbose: u8) {
    let vprint = get_verbose_conditional_print(verbose);

    sudo::with_env(&["USER", "RUST_BACKTRACE"]).unwrap();

    let confirm = prompt::get_string(
        format!("Are you sure you want to remove {}? (Y/n) [n]: ", domain).as_str(),
    ).to_lowercase();

    if confirm != "y" {
        return;
    }

    // remove /etc/hosts entry
    let hosts_file = fs::read_to_string("/etc/hosts");
    match hosts_file {
        Ok(content) => {
            let modified = content.replace(
                format!("127.0.0.1 {}", domain).as_str(),
                ""
            );

            let write = fs::write("/etc/hosts", modified);
            match write {
                Ok(_) => {
                    println!("Hosts file updated");
                },
                Err(err) => {
                    eprintln!("Failed to write to /etc/hosts: {}", err);
                }
            }
        },
        Err(err) => {
            eprintln!("Failed to read hosts file: {}", err);
        }
    }

    handle_remove_site_file_removal(
        format!("/var/www/html/{}", domain).as_str()
    );

    handle_remove_site_file_removal(
        format!("/etc/apache2/sites-enabled/{}.conf", domain).as_str()
    );

    handle_remove_site_file_removal(
        format!("/etc/apache2/sites-available/{}.conf", domain).as_str()
    );

    // remove database
    // prompt first to confirm database removal
    let db_confirm = prompt::get_string("Would you like to remove the database? (Y/n) [n]: ");
    if db_confirm.to_lowercase() != "y" {
        return;
    }

    let db_pass = prompt::get_string("Enter MySQL root password: ");
    let db_name = slugify(domain);

    match Builder::new_current_thread().enable_all().build() {
        Ok(rt) => {
            let db_result = rt.block_on(
                db::drop_database(
                    &db_name,
                    db::get_default_config(&db_pass)
                )
            );

            match db_result {
                Ok(_) => {
                    vprint(1, "Database dropped")
                },
                Err(err) => {
                    eprintln!("Database error: {}", err);
                }
            }
        },
        Err(e) => {
            eprintln!("Thread builder error: {}", e);
        }
    }

    let result = restart_apache();
    if let Err(e) = result {
        eprintln!("{}", e);
    } else {
        vprint(1, "Apache restarted successfully")
    }
}

pub fn restart_site(
    domain: &str,
    verbose: u8,
) {
    let vprint = get_verbose_conditional_print(verbose);

    match find_site_php_version(&domain, verbose) {
        Ok(Some(php_fpm_string)) => {
            vprint(1, format!("Found: {}", php_fpm_string).as_str());

            // restart php
            let result = restart_service(php_fpm_string.as_str());
            if let Err(e) = result {
                eprintln!("{}", e);
            } else {
                vprint(0, format!("Restarted {}", php_fpm_string).as_str());
            }
        },
        Ok(None) => {
            eprintln!("Failed to find site php version: {}", domain);
        }
        Err(err) => {
            eprintln!("Failed to find site php version: {}", err);
        }
    };

    let result = restart_apache();
    if let Err(e) = result {
        eprintln!("{}", e);
    } else {
        vprint(0, "Restarted Apache");
    }
}

fn setup_wp(
    site_path: &str,
    db_name: &str,
    db_user: &str,
    db_pass: &str,
    verbose: u8,
) -> Result<(), io::Error> {
    let vprint = get_verbose_conditional_print(verbose);

    // get htaccess
    let htaccess = template::get_wp_htaccess_template();

    let htaccess_path = format!("{}/.htaccess", site_path);

    let write_result = fs::write(htaccess_path, &htaccess);
    match write_result {
        Ok(_) => {
            vprint(1, "WP htaccess created")
        }
        Err(err) => {
            eprintln!("Failed to write htaccess: {}", err);
        }
    }

    vprint(1, "Downloading WordPress archive");

    let download_path = "/etc/latest.tar.gz";
    let download_response = http::download_remote_file(
        "https://wordpress.org/latest.tar.gz",
        download_path
    );

    match download_response {
        Ok(_) => {
            vprint(1, "WP archive download complete");
        }
        Err(err) => {
            eprintln!("Failed to download WP archive: {}", err);

            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to download WP archive: {}", err)
            ))
        }
    }

    // unpack archive
    let tar_gz = match File::open(download_path) {
        Ok(file) => file,
        Err(err) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to open WP archive: {}", err)
            ))
        }
    };

    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    let prefix = Path::new("wordpress");
    let target_base = Path::new(site_path);

    // move files to site_path
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let stripped_path = match path.strip_prefix(prefix) {
            Ok(p) => {
                if p.as_os_str().is_empty() {
                    continue;
                }

                p
            },
            Err(_) => &path,
        };

        let dest_path = target_base.join(stripped_path);
        entry.unpack(&dest_path)?;
    }

    // replace db_creds in wp-config-sample.php
    let wp_config_file = fs::read_to_string(format!("{}/wp-config-sample.php", site_path));
    match wp_config_file {
        Ok(mut content) => {
            content = content.replace("localhost", "127.0.0.1");
            content = content.replace("database_name_here", &db_name);
            content = content.replace("username_here", &db_user);
            content = content.replace("password_here", &db_pass);

            let result = fs::write(format!("{}/wp-config.php", site_path), content);
            match result {
                Ok(_) => {
                    vprint(1, "WP config updated")
                }
                Err(err) => {
                    eprintln!("Failed to write wp-config: {}", err);
                }
            }
        }
        Err(err) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidFilename,
                format!("Failed to read WP config file: {}", err)
            ))
        }
    }

    // TODO check ownership and permissions

    Ok(())
}

fn restart_apache() -> Result<(), io::Error> {
    restart_service("apache2")
}

fn restart_service(
    service: &str,
) -> Result<(), io::Error> {
    let output_result = ShellCommand::new("service")
        .arg(service)
        .arg("restart")
        .output();

    match output_result {
        Ok(output) => {
            if !output.status.success() {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to restart service: {}", output.status)
                ))
            } else {
                Ok(())
            }
        },
        Err(e) => {
            Err(e)
        }
    }
}

fn find_site_php_version(
    domain: &str,
    verbose: u8,
) -> io::Result<Option<String>> {
    let vprint = get_verbose_conditional_print(verbose);

    let site_config_path = format!("/etc/apache2/sites-available/{}.conf", domain);

    let file = File::open(site_config_path)?;
    let reader = io::BufReader::new(file);

    let result = Regex::new(r"php\d+(?:\.\d+)*-fpm");
    let re = match result {
        Ok(re) => re,
        Err(err) => {
            vprint(0, format!("Error compiling regex for php version: {}", err).as_str());
            return Ok(None)
        }
    };

    for line in reader.lines() {
        let line = line?;

        if let Some(re_match) = re.find(&line) {
            return Ok(Some(re_match.as_str().to_string()));
        }
    }

    Ok(None)
}

fn handle_remove_site_file_removal(path: &str) {
    let result = fs::remove_file(path);

    match result {
        Ok(_) => {
            println!("Deleted {}", path);
        }
        Err(err) => {
            eprintln!("Failed to delete {}: {}", path, err);
        }
    }
}

fn get_verbose_conditional_print(level: u8) -> impl Fn(u8, &str) {
    // move to give ownership of the captured values to the closure
    move | threshold, msg | {
        if level >= threshold {
            println!("{}", msg);
        }
    }
}

// replace with underscores
fn slugify(s: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9_]");

    match re {
        Ok(regex) => {
            regex.replace_all(s, "_").into_owned()
        },
        Err(_) => {
            s.to_string()
        },
    }
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
// The input is empty, so it returns the CWD
// A valid existing path is entered
// Tha absolute path with trailing slash trimmed is returned
fn prompt_for_directory_input(msg: &str) -> String {
    loop {
        let mut input = prompt::get_string(msg);

        let current_dir = env::current_dir();

        match current_dir {
            // panic here is fine.
            // if we're not getting path's right, we shouldn't continue
            Ok(current_dir) => {
                // if input is empty, return CWD
                if input.is_empty() {
                    if current_dir.is_dir() {
                        return normalize_path(
                            current_dir.to_str().unwrap()
                        ).to_string();
                    }
                }

                if !input.starts_with("/") {
                    let dir_str = current_dir.to_str()
                        .unwrap()
                        .to_string();
                    input = format!("{}/{}", dir_str, input);
                }

                if Path::new(&input).exists() {
                    return normalize_path(&input).to_string();
                } else {
                    println!("{} does not exist", input);
                }
            },
            Err(err) => {
                panic!("Directory read error: {}", err);
            }
        }
    }
}

// Trim any trailing slash from paths
fn normalize_path(path: &str) -> &str {
    path.trim_end_matches("/")
}
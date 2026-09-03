
pub fn get_apache_config_template(
    domain: &str,
    doc_root: &str,
    php_version: &str,
) -> String {
    format!(
        include_str!("templates/apache.conf"),
        domain = domain,
        doc_root = doc_root,
        php_version = php_version
    )
}

pub fn get_wp_htaccess_template() -> String {
    include_str!("templates/wp.htaccess").to_string()
}
use sqlx::{AnyPool};

pub enum DbDriver {
    Mysql,
    Postgres,
    Sqlite,
}

pub struct DbConfig<'a> {
    pub driver: DbDriver,
    pub host: &'a str,
    pub port: u16,
    pub user: &'a str,
    pub pass: &'a str,
    pub debug: bool,
}


pub fn get_default_config(pass: &str) -> DbConfig<'_> {
    DbConfig{
        driver: DbDriver::Mysql,
        host: "127.0.0.1",
        port: 3306,
        user: "root",
        pass,
        debug: true,
    }
}

pub async fn create_database(
    name: &str,
    config: DbConfig<'_>,
) -> Result<(), sqlx::Error> {
    let dsn = get_dsn(&config);

    if config.debug {
        println!("Connecting: {}", dsn);
    }

    let pool = connect(&dsn).await?;

    let statement = format!("CREATE DATABASE {}", name);

    if config.debug {
        println!("Creating database: {}", statement);
    }

    sqlx::query(
        sqlx::AssertSqlSafe(statement)
    )
        .execute(&pool)
        .await
        .map(|_| ())
}

pub async fn drop_database(
    name: &str,
    config: DbConfig<'_>,
) -> Result<(), sqlx::Error> {
    let dsn = get_dsn(&config);

    if config.debug {
        println!("Connecting: {}", dsn);
    }

    let pool = connect(&dsn).await?;

    let statement = format!("DROP DATABASE {}", name);

    if config.debug {
        println!("Dropping database: {}", statement);
    }

    sqlx::query(
        sqlx::AssertSqlSafe(statement)
    )
        .execute(&pool)
        .await
        .map(|_| ())
}

async fn connect(dsn: &str) -> Result<AnyPool, sqlx::Error> {
    // register drivers
    sqlx::any::install_default_drivers();

    let pool = AnyPool::connect(&dsn).await?;

    Ok(pool)
}

fn get_driver_string(config: &DbConfig) -> String {
    let driver = match config.driver {
        DbDriver::Mysql => "mysql",
        DbDriver::Postgres => "postgres",
        DbDriver::Sqlite => "sqlite",
    };
    driver.to_string()
}

fn get_dsn(config: &DbConfig) -> String {
    format!(
        "{}://{}:{}@{}:{}/", 
        get_driver_string(config), 
        config.user, 
        config.pass, 
        config.host, 
        config.port,
    )
}
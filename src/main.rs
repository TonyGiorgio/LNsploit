mod application;
mod models;
mod router;
mod screens;

use anyhow::Result;
use application::Application;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use chrono::Utc;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lightning::util::logger::{Logger, Record};
use serde::Deserialize;
use std::fs;
use std::sync::Arc;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Debug, Deserialize)]
struct Config {
    db: DbConfig,
    bitcoind: BitcoindConfig,
    logger: LoggerConfig,
}

#[derive(Debug, Deserialize)]
struct DbConfig {
    connection: String,
}

#[derive(Debug, Deserialize)]
struct BitcoindConfig {
    rpc_host: String,
    rpc_port: u16,
    rpc_username: String,
    rpc_password: String,
}

#[derive(Debug, Deserialize)]
struct LoggerConfig {
    location: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // config parsing
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or(String::from("config.yaml"));
    let config_file = std::fs::File::open(config_path)
        .expect("cannot open config file, make sure one exists or is specific");
    let config: Config =
        serde_yaml::from_reader(config_file).expect("yaml config was not well formatted");

    // DB management
    let manager = ConnectionManager::<SqliteConnection>::new(config.db.connection);
    let pool = Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool");
    let connection = &mut pool.get().unwrap();
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("migrations could not run");

    // bitcoind client
    let bitcoind_client = Client::new(
        String::as_str(&format!(
            "{}:{}",
            config.bitcoind.rpc_host, config.bitcoind.rpc_port
        )),
        Auth::UserPass(config.bitcoind.rpc_username, config.bitcoind.rpc_password),
    )
    .expect("could not create bitcoind client");
    let _best_block_hash = bitcoind_client
        .get_best_block_hash()
        .expect("could not get block from bitcoind");

    // global logger
    let logger = Arc::new(FilesystemLogger::new(config.logger.location.clone()));

    // main application
    let app = Application::new(pool, bitcoind_client, logger).await?;

    if let Err(e) = app.run().await {
        println!("error starting the application: {}", e);
    };

    Ok(())
}

pub struct FilesystemLogger {
    location: String,
}
impl FilesystemLogger {
    pub(crate) fn new(location: String) -> Self {
        Self { location }
    }
}
impl Logger for FilesystemLogger {
    fn log(&self, record: &Record) {
        if record.level.to_string() != "INFO" && record.level.to_string() != "WARN" {
            return;
        }
        let raw_log = record.args.to_string();
        let log = format!(
            "{} {:<5} [{}:{}] {}\n",
            // Note that a "real" lightning node almost certainly does *not* want subsecond
            // precision for message-receipt information as it makes log entries a target for
            // deanonymization attacks. For testing, however, its quite useful.
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level.to_string(),
            record.module_path,
            record.line,
            raw_log
        );
        let logs_file_path = format!("{}", self.location.clone());
        std::io::Write::write_all(
            &mut fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(logs_file_path)
                .unwrap(),
            log.as_bytes(),
        )
        .unwrap();
    }
}

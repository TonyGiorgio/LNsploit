mod application;
mod models;
mod router;
mod screens;

use anyhow::Result;
use application::Application;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::Deserialize;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Debug, Deserialize)]
struct Config {
    db: DbConfig,
}

#[derive(Debug, Deserialize)]
struct DbConfig {
    connection: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or(String::from("config.yaml"));
    let config_file = std::fs::File::open(config_path)
        .expect("cannot open config file, make sure one exists or is specific");
    let config: Config =
        serde_yaml::from_reader(config_file).expect("yaml config was not well formatted");

    let manager = ConnectionManager::<SqliteConnection>::new(config.db.connection);
    let pool = Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool");
    let connection = &mut pool.get().unwrap();
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("migrations could not run");

    let app = Application::new(pool).await?;

    if let Err(e) = app.run().await {
        println!("error starting the application: {}", e);
    };

    Ok(())
}

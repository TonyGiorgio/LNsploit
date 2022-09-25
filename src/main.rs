mod application;
mod models;
mod router;
mod screens;

use anyhow::Result;
use application::Application;
use serde::Deserialize;

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

    println!(
        "loading up database connection information from config: {:?}",
        config.db.connection
    );

    let app = Application::new().await?;

    if let Err(e) = app.run().await {
        println!("error starting the application: {}", e);
    };

    Ok(())
}

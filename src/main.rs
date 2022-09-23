mod application;
mod models;
mod router;
mod screens;

use anyhow::Result;
use application::Application;

#[tokio::main]
async fn main() -> Result<()> {
    let app = Application::new().await?;

    if let Err(e) = app.run().await {
        println!("error starting the application: {}", e);
    };

    Ok(())
}

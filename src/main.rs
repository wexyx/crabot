use std::time::Duration;

use anyhow::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // middleware::init().await?;
    env_logger::init();
    ai::init();
    ai::demo().await?;
    Ok(())
}

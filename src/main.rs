use crabot::middleware;
use anyhow::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    middleware::init().await?;
    env_logger::init();

    Ok(())
}

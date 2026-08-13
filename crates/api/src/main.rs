use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let addr = std::env::var("CRABOT_API_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    println!("crabot api listening on http://{}", addr);
    crabot_api::serve(&addr).await
}

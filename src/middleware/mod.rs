use anyhow::Error;

pub mod database;
pub mod rig;

pub async fn init() -> Result<(), Error> {
    database::Database::init("").await?;

    Ok(())
}
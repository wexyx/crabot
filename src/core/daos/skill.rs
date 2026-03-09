use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};

pub async fn create(skill: models::skill::Skill) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::skill::Skill::insert(client.as_ref(), &skill).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}


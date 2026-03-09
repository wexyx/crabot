use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(skill: models::skill::Skill) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::skill::Skill::insert(client.as_ref(), &skill).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::skill::Skill>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::skill::Skill::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(skill: models::skill::Skill) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::skill::Skill::update_by_map(client.as_ref(), &skill, value! {"id": skill.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut skill) = get(id).await? {
        skill.is_deleted = true;
        update(skill).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::skill::Skill>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::skill::Skill::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}


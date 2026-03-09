use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(agent: models::llm::Agent) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::llm::Agent::insert(client.as_ref(), &agent).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::llm::Agent>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::llm::Agent::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(agent: models::llm::Agent) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::llm::Agent::update_by_map(client.as_ref(), &agent, value! {"id": agent.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut agent) = get(id).await? {
        agent.is_deleted = true;
        update(agent).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::llm::Agent>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::llm::Agent::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}
use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(message: models::message::Message) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::message::Message::insert(client.as_ref(), &message).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::message::Message>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::message::Message::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(message: models::message::Message) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::message::Message::update_by_map(client.as_ref(), &message, value! {"id": message.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut message) = get(id).await? {
        message.is_deleted = true;
        update(message).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::message::Message>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::message::Message::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}
use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(session: models::session::Session) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::session::Session::insert(client.as_ref(), &session).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::session::Session>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::session::Session::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(session: models::session::Session) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::session::Session::update_by_map(client.as_ref(), &session, value! {"id": session.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut session) = get(id).await? {
        session.is_deleted = true;
        update(session).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::session::Session>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::session::Session::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}
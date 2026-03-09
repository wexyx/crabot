use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(worker: models::worker::Worker) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::worker::Worker::insert(client.as_ref(), &worker).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::worker::Worker>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::worker::Worker::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(worker: models::worker::Worker) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::worker::Worker::update_by_map(client.as_ref(), &worker, value! {"id": worker.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut worker) = get(id).await? {
        worker.is_deleted = true;
        update(worker).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::worker::Worker>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::worker::Worker::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}
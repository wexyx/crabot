use ::anyhow::Error;

use crate::{biz_err, core::models, middleware};
use rbs::value;

pub async fn create(job: models::job::Job) -> Result<Option<u64>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::job::Job::insert(client.as_ref(), &job).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.last_insert_id.as_u64())
}

pub async fn get(id: u64) -> Result<Option<models::job::Job>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::job::Job::select_by_map(client.as_ref(), value! {"id": id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result.into_iter().next())
}

pub async fn update(job: models::job::Job) -> Result<(), Error> {
    let client = middleware::database::instance()?.client();
    models::job::Job::update_by_map(client.as_ref(), &job, value! {"id": job.id}).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(())
}

pub async fn delete(id: u64) -> Result<(), Error> {
    if let Some(mut job) = get(id).await? {
        job.is_deleted = true;
        update(job).await?;
    }
    Ok(())
}

pub async fn list() -> Result<Vec<models::job::Job>, Error> {
    let client = middleware::database::instance()?.client();
    let result = models::job::Job::select_all(client.as_ref()).await.map_err(|e|biz_err!(e.to_string()))?;
    Ok(result)
}
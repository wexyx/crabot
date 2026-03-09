use anyhow::Error;
use std::pin::Pin;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::error::Elapsed;

use crate::biz_err;

#[macro_export]
macro_rules! context_task {
    ($future:expr) => {{
        use crate::utils::task_local;

        let task_local_data = task_local::get_task_local();
        task_local::TASK_LOCAL.scope(std::cell::RefCell::new(Some(task_local_data)), $future)
    }};
}

pub fn spawn<T, F: std::future::Future<Output = T>>(future: F) -> Future<T>
where
    T: Send + 'static,
    F: Send + 'static,
{
    spawn_timeout(future, Duration::from_secs(2))
}

#[allow(dead_code)]
pub fn spawn_result<T, F: std::future::Future<Output = Result<T, Error>>>(
    future: F,
) -> ResultFuture<T>
where
    T: Send + 'static,
    F: Send + 'static,
{
    spawn_result_timeout(future, Duration::from_secs(2))
}

pub fn spawn_timeout<T, F: std::future::Future<Output = T>>(
    future: F,
    timeout: Duration,
) -> Future<T>
where
    T: Send + 'static,
    F: Send + 'static,
{
    Future::new(timeout, future)
}

#[allow(dead_code)]
pub fn spawn_result_timeout<T, F: std::future::Future<Output = Result<T, Error>>>(
    future: F,
    timeout: Duration,
) -> ResultFuture<T>
where
    T: Send + 'static,
    F: Send + 'static,
{
    ResultFuture::new(timeout, future)
}
pub struct Future<E>
where
    E: Send + 'static,
{
    future: JoinHandle<Result<E, Elapsed>>,
}

impl<E> Future<E>
where
    E: Send + 'static,
{
    pub fn new<F>(timeout: Duration, future: F) -> Self
    where
        F: std::future::Future<Output = E> + Send + 'static,
    {
        let context_future = context_task!(future);
        let timeout_future = tokio::spawn(tokio::time::timeout(timeout, context_future));

        return Self {
            future: timeout_future,
        };
    }
}

impl<E> std::future::Future for Future<E>
where
    E: Send + 'static,
{
    type Output = Result<E, Error>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Pin::new(&mut self.get_mut().future)
            .poll(cx)
            .map(|result| match result {
                Ok(first_join_result) => {
                    first_join_result.map_err(|e| biz_err!(format!("timeout: {}", e.to_string())))
                }
                Err(err) => Err(biz_err!(err.to_string())),
            })
    }
}

pub struct ResultFuture<E>
where
    E: Send + 'static,
{
    future: JoinHandle<Result<Result<E, Error>, Elapsed>>,
}

impl<E> ResultFuture<E>
where
    E: Send + 'static,
{
    pub fn new<F>(timeout: Duration, future: F) -> Self
    where
        F: std::future::Future<Output = Result<E, Error>> + Send + 'static,
    {
        let context_future = context_task!(future);
        let timeout_future = tokio::spawn(tokio::time::timeout(timeout, context_future));

        return Self {
            future: timeout_future,
        };
    }
}

impl<E> std::future::Future for ResultFuture<E>
where
    E: Send + 'static,
{
    type Output = Result<E, Error>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Pin::new(&mut self.get_mut().future)
            .poll(cx)
            .map(|result| match result {
                Ok(first_join_result) => match first_join_result {
                    Ok(timeout_result) => timeout_result,
                    Err(err) => Err(biz_err!(format!("timeout: {}", err.to_string()))),
                },
                Err(err) => Err(biz_err!(err.to_string())),
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::task_local;
    use super::*;
    #[tokio::test]
    async fn test_task_local() {
        let r = spawn(async {
            task_local::set("hello_world", "hello_world_ddddd".to_string());

            let r2 = spawn(async {
                task_local::set("hello_world222", "hello_world222_jjjjj".to_string());
                let hello_world = task_local::get::<String>("hello_world");
                assert!(hello_world.is_some());
                assert!(hello_world.unwrap_or_default().eq("hello_world_ddddd"));

                let hello_world222 = task_local::get::<String>("hello_world222");
                assert!(hello_world222.is_some());
                assert!(hello_world222.unwrap_or_default().eq("hello_world222_jjjjj"));
            }).await;

            assert!(r2.is_ok());

            // 当前特性，子任务数据也能被父任务读取到，内部采用读写锁，所以线程安全
            let hello_world222 = task_local::get::<String>("hello_world222");
            assert!(hello_world222.is_some());
        }).await;

        assert!(r.is_ok());
    }
}
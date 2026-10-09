use crate::management::Manager;
use std::sync::Arc;
use tokio::{
    sync::{Mutex, oneshot},
    task::JoinHandle,
};
struct Running {
    address: String,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}
#[derive(Default)]
pub(crate) struct WebServer {
    running: Mutex<Option<Running>>,
}
impl WebServer {
    pub(crate) async fn start(
        &self,
        manager: Arc<Manager>,
        address: &str,
    ) -> Result<String, String> {
        let access = super::web_access::WebAccess::from_env()?;
        access.validate_bind(address.parse().map_err(|_| "invalid Server address")?)?;
        let mut running = self.running.lock().await;
        if let Some(current) = running.as_ref() {
            let requested = address
                .parse::<std::net::SocketAddr>()
                .map_err(|_| "invalid Server address")?;
            if requested.port() != 0 && address != current.address {
                return Err(format!(
                    "Server already running at {}; stop it before choosing a different port",
                    current.address
                ));
            }
            return Ok(format!("http://{}", current.address));
        }
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|e| format!("Cannot start Server at {address}: {e}. Choose a different port (or port 0 for automatic allocation)."))?;
        let address = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .to_string();
        manager
            .core()
            .state()
            .store
            .transaction(|data| {
                data.set(
                    "instance_settings",
                    "web",
                    serde_json::json!({"address":address}),
                );
                Ok(())
            })
            .await?;
        let router =
            super::routes::router_with_manager(manager.core().state().clone(), manager, access);
        let (stop, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            {
                eprintln!("Server: {error}");
            }
        });
        *running = Some(Running {
            address: address.clone(),
            stop,
            task,
        });
        Ok(format!("http://{address}"))
    }
    pub(crate) async fn stop(&self) {
        if let Some(mut running) = self.running.lock().await.take() {
            let _ = running.stop.send(());
            if tokio::time::timeout(std::time::Duration::from_secs(2), &mut running.task)
                .await
                .is_err()
            {
                running.task.abort();
            }
        }
    }
    pub(crate) async fn address(&self) -> Option<String> {
        self.running
            .lock()
            .await
            .as_ref()
            .map(|r| r.address.clone())
    }
}

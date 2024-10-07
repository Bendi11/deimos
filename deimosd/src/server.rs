use std::{process::ExitCode, sync::Arc};

use api::{ApiConfig, ApiInitError, ApiState};
use deimos_shared::ContainerStatusNotification;
use docker::{DockerConfig, DockerState};
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;
use upnp::Upnp;

mod docker;
mod api;
mod upnp;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos {
    docker: DockerState,
    api: ApiState,
    upnp: Upnp,
    status: tokio::sync::broadcast::Sender<ContainerStatusNotification>
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeimosConfig {
    pub docker: DockerConfig,
    pub api: ApiConfig,
}

impl Deimos {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    pub async fn new(config: DeimosConfig) -> Result<Arc<Self>, ServerInitError> {
        let upnp = Upnp::new().await?;
        let docker = DockerState::new(config.docker)
            .await
            .map_err(ServerInitError::Docker)?;

        let api = ApiState::new(&upnp, config.api).await?;

        let (status, _) = tokio::sync::broadcast::channel(2);

        Ok(
            Arc::new(Self {
                docker,
                api,
                upnp,
                status
            })
        )
    }
    
    /// Run the server until an interrupt signal is received or a fatal error occurs
    pub async fn run(self: Arc<Self>) -> ExitCode {
        let cancel = CancellationToken::new();
        
        let cancel_copy = cancel.clone();
        let this = self.clone();
        let api_server = tokio::task::spawn(async move {
            if let Err(e) = this.serve_api(cancel_copy).await {
                tracing::error!("Failed to serve gRPC API: {e}");
            }
        });

        #[cfg(unix)]
        {
            let mut close = match tokio::signal::unix::signal(SignalKind::interrupt()) {
                Ok(sig) => sig,
                Err(e) => {
                    tracing::error!("Failed to create SIGINT handler: {e}");
                    return ExitCode::FAILURE
                }
            };

            if let Some(()) = close.recv().await {
                tracing::info!("Got SIGINT, shutting down deimosd");
                cancel.cancel();
            } 
        }

        let _ = tokio::join! {
            api_server
        };

        ExitCode::SUCCESS
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to initialize Docker service: {0}")]
    Docker(docker::DockerInitError),
    #[error("Failed to initialize UPNP state: {0}")]
    Upnp(#[from] upnp::UpnpInitError),
}

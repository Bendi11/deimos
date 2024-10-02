use std::{process::ExitCode, sync::Arc};

use api::{ApiConfig, ApiInitError};
use docker::{DockerConfig, DockerState};
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;

mod docker;
mod api;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos {
    docker: DockerState,
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
        let docker = DockerState::new(config.docker)
            .await
            .map_err(ServerInitError::Docker)?;

        Ok(
            Arc::new(Self {
                docker,
            })
        )
    }
    
    /// Run the server until an interrupt signal is received or a fatal error occurs
    pub async fn run(self: Arc<Self>) -> ExitCode {
        let cancel = CancellationToken::new();

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
                let _ = tasks.await;
            } 
        }
        #[cfg(not(unix))]
        tasks.await;

        ExitCode::SUCCESS
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to initialize Docker service: {0}")]
    Docker(docker::DockerInitError),
}

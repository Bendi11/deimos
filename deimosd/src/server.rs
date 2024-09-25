use std::sync::Arc;

use crate::services::{
    api::{ApiConfig, ApiInitError, ApiService},
    docker::{DockerConfig, DockerService},
};
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos;

#[derive(Debug, serde::Deserialize)]
pub struct DeimosConfig {
    pub docker: DockerConfig,
    pub api: ApiConfig,
}

impl Deimos {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    pub async fn start(config: DeimosConfig) -> Result<(), ServerInitError> {
        let docker = Arc::new(
            DockerService::new(config.docker)
                .await
                .map_err(ServerInitError::Docker)?,
        );
        let api = Arc::new(ApiService::new(config.api, docker.clone()).await?);

        let cancel = CancellationToken::new();

        let tasks = async {
            tokio::join! {
                tokio::spawn(api.run(cancel.clone())),
                tokio::spawn(docker.run(cancel.clone()))
            }
        };

        #[cfg(unix)]
        {
            let mut close = tokio::signal::unix::signal(SignalKind::terminate())
                .map_err(ServerInitError::Signal)?;
            tokio::select! {
                _ = close.recv() => {
                    cancel.cancel();
                },
                _ = tasks => {}
            };
        }
        #[cfg(not(unix))]
        tasks.await;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to create SIGTERM listener: {0}")]
    Signal(std::io::Error),
    #[error("Failed to initialize Docker service: {0}")]
    Docker(crate::services::docker::DockerServiceInitError),
}

use std::{process::ExitCode, sync::Arc};

use api::{ApiConfig, ApiInitError, ApiState};
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;
use upnp::Upnp;

use crate::pod::manager::{PodManager, PodManagerConfig, PodManagerInitError};

mod api;
mod upnp;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos {
    pods: PodManager,
    api: ApiState,
    upnp: Upnp,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeimosConfig {
    pub pod: PodManagerConfig,
    pub api: ApiConfig,
}

impl Deimos {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    pub async fn new(config: DeimosConfig) -> Result<Arc<Self>, ServerInitError> {
        let upnp = Upnp::new().await?;
        let pods = PodManager::init(config.pod).await?;
        let api = ApiState::new(&upnp, config.api).await?;

        Ok(Arc::new(Self { pods, api, upnp }))
    }

    /// Run the server until an interrupt signal is received or a fatal error occurs
    pub async fn run(self: Arc<Self>) -> ExitCode {
        let cancel = CancellationToken::new();

        let api_server = tokio::task::spawn(self.clone().api_task(cancel.clone()));
        let upnp = tokio::task::spawn(self.clone().upnp_task(cancel.clone()));

        #[cfg(unix)]
        {
            let mut close = match tokio::signal::unix::signal(SignalKind::interrupt()) {
                Ok(sig) => sig,
                Err(e) => {
                    tracing::error!("Failed to create SIGINT handler: {e}");
                    return ExitCode::FAILURE;
                }
            };

            let mut term = match tokio::signal::unix::signal(SignalKind::terminate()) {
                Ok(sig) => sig,
                Err(e) => {
                    tracing::error!("Failed to create SIGTERM handler: {e}");
                    return ExitCode::FAILURE;
                }
            };

            tokio::select! {
                _ = close.recv() => {
                    tracing::info!("Got SIGINT");
                },
                _ = term.recv() => {
                    tracing::info!("Got SIGTERM");
                },
            };

            cancel.cancel();
        }

        let _ = tokio::join! {
            api_server,
            upnp,
        };

        ExitCode::SUCCESS
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to initialize Docker service: {0}")]
    Pod(#[from] PodManagerInitError),
    #[error("Failed to initialize UPNP state: {0}")]
    Upnp(#[from] upnp::UpnpInitError),
}

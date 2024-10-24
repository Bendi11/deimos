use std::{process::ExitCode, sync::Arc};

use api::{ApiConfig, ApiInitError, ApiState};
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;
use upnp::{Upnp, UpnpReceiver};

use crate::pod::{PodManager, PodManagerConfig, PodManagerInitError};


mod api;
pub mod upnp;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos {
    upnp: Upnp,
    pub pods: PodManager,
    api: ApiState,
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
    /// Then run the server until an interrupt signal is received or a fatal error occurs
    pub async fn run(config: DeimosConfig) -> Result<(), DeimosRunError> {
        let (upnp, upnp_rx) = Upnp::new().await?;
        let api = ApiState::new(&upnp, config.api).await?;
        let pods = PodManager::new(config.pod, upnp.clone()).await?;
        let this = Arc::new(
            Self {
                pods,
                api,
                upnp,
            }
        );

        let cancel = CancellationToken::new();
        let upnp = tokio::task::spawn(this.clone().upnp_task(upnp_rx, cancel.clone()));
        let api_server = tokio::task::spawn(this.clone().api_task(cancel.clone()));
        let pods = tokio::task::spawn(this.clone().pod_task(cancel.clone()));

        #[cfg(unix)]
        {
            let (mut int, mut term) = match (
                tokio::signal::unix::signal(SignalKind::interrupt()),
                tokio::signal::unix::signal(SignalKind::terminate()),
            ) {
                (Ok(int), Ok(term)) => (int, term),
                (Err(e), _) | (_, Err(e)) => {
                    cancel.cancel();

                    return Err(
                        DeimosRunError::Signal(e)
                    )
                }
            };

            tokio::select! {
                _ = int.recv() => {
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
            pods,
        };
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeimosRunError {
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to initialize Docker service: {0}")]
    Pod(#[from] PodManagerInitError),
    #[error("Failed to initialize UPNP state: {0}")]
    Upnp(#[from] upnp::UpnpInitError),
    #[error("Failed to subscribe to signals: {0}")]
    Signal(#[source] std::io::Error),
}

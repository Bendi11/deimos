use std::{path::PathBuf, sync::Arc};

use api::{ApiConfig, ApiInitError, ApiPersistent, ApiState};
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use upnp::{Upnp, UpnpConfig};

use crate::pod::{PodManager, PodManagerConfig, PodManagerInitError};


mod api;
pub mod upnp;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Deimos {
    pub pods: PodManager,
    upnp: Upnp,
    api: ApiState,
}


#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeimosConfig {
    /// Path to write a save files to
    pub save_path: PathBuf,
    /// Configuration for the pod manager
    pub pod: PodManagerConfig,
    /// Configuration for the public and private API servers
    pub api: ApiConfig,
    /// Configuration for the UPnP client
    #[serde(default)]
    pub upnp: UpnpConfig,
}

/// Persistent state written to a save file specified in the config [DeimosConfig::save_path]
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeimosPersistent {
    api: ApiPersistent,
}

impl Deimos {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    /// Then run the server until an interrupt signal is received or a fatal error occurs
    pub async fn run(config: DeimosConfig) -> Result<(), DeimosRunError> {
        let persistent = match std::fs::File::open(&config.save_path) {
            Ok(file) => serde_json::from_reader::<_, DeimosPersistent>(file)?,
            Err(e) => {
                tracing::warn!("Failed to load save file from {}: {}", config.save_path.display(), e);
                Default::default()
            }
        };

        let (upnp, upnp_rx) = Upnp::new(config.upnp).await?;
        let api = ApiState::load(persistent.api, config.api, &upnp).await?;
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

        let persistent = DeimosPersistent {
            api: this.api.save()
        };

        serde_json::to_writer(
            std::fs::File::create(&config.save_path)
                .map_err(|err| DeimosRunError::SavePersistent { path: config.save_path.clone(), err })?,
            &persistent
        )?;
        
        Ok(())
    }

    /// Monitor events received from the local Docker instance
    pub async fn pod_task(self: Arc<Self>, cancel: CancellationToken) {
        let mut events = self.pods.eventloop();
        

        while let Some((pod, action)) = tokio::select! {
            _ = cancel.cancelled() => None,
            v = events.next() => v,
        } {
            let this = self.clone();
            tokio::task::spawn(async move {
                this.pods.handle_event(pod, action).await;
            });
        }

        self.pods.disable_all().await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeimosRunError {
    #[error("Failed to create save file {}: {}", path.display(), err)]
    SavePersistent {
        path: PathBuf,
        err: std::io::Error,
    },
    #[error("Failed to serialize persistent data: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Failed to initialize API server: {0}")]
    Api(#[from] ApiInitError),
    #[error("Failed to initialize Docker service: {0}")]
    Pod(#[from] PodManagerInitError),
    #[error("Failed to initialize UPNP state: {0}")]
    Upnp(#[from] upnp::UpnpInitError),
    #[error("Failed to subscribe to signals: {0}")]
    Signal(#[source] std::io::Error),
}

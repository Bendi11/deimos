use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use chacha20poly1305::{aead::OsRng, ChaCha20Poly1305, KeyInit};
use deimos_shared::key;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use super::docker::DockerService;

/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiService {
    docker: Arc<DockerService>,
    listener: TcpListener,
}

/// Configuration used to initialize and inform the Deimos API service
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    pub bind: SocketAddr,
    #[serde(default)]
    pub upnp: bool,
    pub keyfile: PathBuf,
}

impl ApiService {
    /// Load the Deimos API service configuration and store a handle to the local Docker instance
    /// to manage containers
    pub async fn new(config: ApiConfig, docker: Arc<DockerService>) -> Result<Self, ApiInitError> {
        if !tokio::fs::try_exists(&config.keyfile).await? {
            tracing::info!(
                "Key file {} does not exist, creating and setting permissions",
                config.keyfile.display()
            );
            let key = ChaCha20Poly1305::generate_key(&mut OsRng);
            key::save_symmetric_pem(&config.keyfile, key).await?;
        }

        let listener = TcpListener::bind(config.bind).await?;

        Ok(Self { docker, listener })
    }

    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {}
}

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to save symmetric key: {0}")]
    Key(#[from] deimos_shared::key::DeimosKeyError),
}

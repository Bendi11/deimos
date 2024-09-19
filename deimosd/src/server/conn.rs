use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use chacha20poly1305::{aead::OsRng, ChaCha20Poly1305, KeyInit};
use deimos_shared::key;
use tokio::net::{TcpListener, TcpStream};

use super::{docker::DockerService, Deimos};


/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiService {
    docker: Arc<DockerService>,
    listener: TcpListener,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiConfig {
    pub bind: SocketAddr,
    #[serde(default)]
    pub upnp: bool,
    pub keyfile: PathBuf,
}

impl ApiService {
    pub async fn new(config: ApiConfig, docker: Arc<DockerService>) -> Result<Self, ApiInitError> {
        if !tokio::fs::try_exists(&config.keyfile).await? {
            tracing::info!("Key file {} does not exist, creating and setting permissions", config.keyfile.display());
            let key = ChaCha20Poly1305::generate_key(&mut OsRng);
            key::save_symmetric_pem(&config.keyfile, key).await?;
        }

        let listener = TcpListener::bind(config.bind).await?;

        Ok(Self {
            docker,
            listener,
        })
    }

    pub async fn run(self: Arc<Self>) -> ! {
        loop {
            
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to save symmetric key: {0}")]
    Key(#[from] deimos_shared::key::DeimosKeyError),
}

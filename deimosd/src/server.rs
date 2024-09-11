use chacha20poly1305::{aead::OsRng, ChaCha20Poly1305, KeyInit};
use conn::Connection;
use tokio::net::TcpListener;

use deimos_shared::key;

use crate::config::DeimosConfig;

pub mod conn;

/// RPC server that listens for TCP connections and spawns tasks to serve clients
pub struct Server {
    listener: TcpListener,
}

impl Server {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    pub async fn new(config: DeimosConfig) -> Result<Self, ServerInitError> {
        if !tokio::fs::try_exists(&config.keyfile).await? {
            tracing::info!("Key file {} does not exist, creating and setting permissions", config.keyfile.display());
            let key = ChaCha20Poly1305::generate_key(&mut OsRng);
            key::save_symmetric_pem(&config.keyfile, key).await?;
        }

        let key = key::load_symmetric_pem(config.keyfile).await?;
        let listener = TcpListener::bind(config.bind).await?;

        Ok(Self {
            listener
        })
    }
    
    /// Await TCP connections on the address specified in the configuration
    pub async fn serve(self) -> ! {
        loop {
            match self.listener.accept().await {
                Ok((conn, addr)) => {
                    tracing::debug!("Accepted connection from {addr}");

                    let conn = Connection::new(conn);
                    tokio::task::spawn(conn.serve());
                },
                Err(e) => {
                    tracing::warn!("Failed to accept TCP connection: {e}");
                }
            }
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to load key: {0}")]
    Key(#[from] deimos_shared::key::DeimosKeyError),
}

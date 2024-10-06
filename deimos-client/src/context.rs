use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use container::CachedContainer;
use deimos_shared::{DeimosServiceClient, QueryContainersRequest};
use http::Uri;
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};
use tonic::{transport::Channel, Code};
use tonic::Status;

pub mod container;
pub mod load;

/// Context shared across the application used to perform API requests on the remote
#[derive(Debug)]
pub struct Context {
    state: ContextState,
    api: Mutex<DeimosServiceClient<Channel>>,
    containers: RwLock<HashMap<String, Arc<CachedContainer>>>,
}

/// Persistent state kept for the [Context]'s connection
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextState {
    #[serde(with="http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    /// Timestamp of the last container synchronization
    pub last_sync: Option<DateTime<Utc>>,
}

impl Context {
    pub const CACHE_DIR_NAME: &str = "deimos";

    /// Create a new lazy API client, which will not attempt a connection until the first API call
    /// is made
    pub async fn new() -> Arc<Self> {
        let state = match Self::load_state() {
            Ok(state) => state,
            Err(e) => {
                tracing::error!("Failed to load application state: {e}");
                ContextState::default()
            }
        };

        let api = Mutex::new(
            DeimosServiceClient::new(
                Channel::builder(state.server_uri.clone())
                    .connect_timeout(state.connect_timeout)
                    .timeout(state.request_timeout)
                    .connect_lazy()
            )
        );

        let containers = RwLock::new(HashMap::new());

        let me = Arc::new(Self {
            state,
            api,
            containers,
        });

        me.load_cached_containers(&Self::cache_directory()).await;

        me
    }

    fn cache_directory() -> PathBuf {
        match dirs::cache_dir() {
            Some(dir) => dir.join(Self::CACHE_DIR_NAME),
            None => PathBuf::from("./deimos-cache"),
        }
    }
}

impl Default for ContextState {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
            last_sync: None,
        }
    }
}

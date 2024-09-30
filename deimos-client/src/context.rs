use std::{sync::Arc, time::Duration};

use container::CachedContainerInfo;
use deimos_shared::{DeimosClient, QueryContainersRequest};
use http::Uri;
use tokio::sync::{RwLock, RwLockReadGuard};
use tonic::{transport::Channel, Code, Status};

pub mod container;

/// Context shared across the application used to perform API requests on the remote
#[derive(Debug)]
pub struct Context {
    state: ContextState,
    api: RwLock<DeimosClient<Channel>>,
    containers: Vec<Arc<CachedContainerInfo>>,
}

/// Persistent state kept for the [Context]'s connection
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextState {
    #[serde(with="http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
}

impl Context {
    /// Create a new lazy API client, which will not attempt a connection until the first API call
    /// is made
    pub async fn new(state: ContextState) -> Self {
        let api = RwLock::new(
            DeimosClient::new(
                Channel::builder(state.server_uri.clone())
                    .connect_timeout(state.connect_timeout)
                    .timeout(state.request_timeout)
                    .connect_lazy()
            )
        );

        let containers = Vec::new();

        Self {
            state,
            api,
            containers,
        }
    }
    
    /// Get an iterator over the currently cached containers
    pub fn containers(&self) -> impl Iterator<Item = Arc<CachedContainerInfo>> + '_ {
        self
            .containers
            .iter()
            .cloned()
    }

        
    /// Get a reference to the client used to issue API requests to the server
    pub async fn api(&self) -> RwLockReadGuard<'_, DeimosClient<Channel>> {
        self.api.read().await
    }
}

impl Default for ContextState {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
        }
    }
}

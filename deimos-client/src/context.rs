use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use container::CachedContainer;
use deimos_shared::DeimosServiceClient;
use http::Uri;
use tokio::sync::{Mutex, RwLock};
use tonic::transport::Channel;

pub mod container;
pub mod load;

/// Context shared across the application used to perform API requests on the remote
#[derive(Debug)]
pub struct Context {
    state: RwLock<ContextState>,
    api: Mutex<DeimosServiceClient<Channel>>,
    containers: RwLock<HashMap<String, Arc<CachedContainer>>>,
}

/// Settings that may be adjusted by the user
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextSettings {
    #[serde(with = "http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
}

/// Persistent state kept for the [Context]'s connection
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextState {
    pub settings: ContextSettings,
    /// Timestamp of the last container synchronization
    pub last_sync: Option<DateTime<Utc>>,
}

impl Context {
    pub const CACHE_DIR_NAME: &str = "deimos";

    pub async fn cleanup(self: Arc<Self>) {
        self.save_state().await;
        self.save_cached_containers().await;
    }

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

        let api = Mutex::new(Self::connect_api(&state).await);
        let state = RwLock::new(state);
        let containers = RwLock::new(HashMap::new());

        let me = Arc::new(Self {
            state,
            api,
            containers,
        });

        me.load_cached_containers(&Self::cache_directory()).await;

        me
    }

    async fn connect_api(state: &ContextState) -> DeimosServiceClient<Channel> {
        DeimosServiceClient::new(
            Channel::builder(state.settings.server_uri.clone())
                .connect_timeout(state.settings.connect_timeout)
                .timeout(state.settings.request_timeout)
                .connect_lazy(),
        )
    }

    /// Reload the current context with the given updated settings
    pub async fn reload_settings(self: Arc<Self>, settings: ContextSettings) {
        let mut old_state = self.state.write().await;
        let mut api = self.api.lock().await;

        let state = ContextState {
            settings,
            last_sync: old_state.last_sync,
        };

        *api = Self::connect_api(&state).await;
        *old_state = state;
    }

    pub async fn containers(self: Arc<Self>) -> Vec<Arc<CachedContainer>> {
        self.containers.read().await.values().cloned().collect()
    }

    /// Get the settings applied to this context
    pub async fn settings(&self) -> ContextSettings {
        self.state.read().await.settings.clone()
    }

    fn cache_directory() -> PathBuf {
        match dirs::cache_dir() {
            Some(dir) => dir.join(Self::CACHE_DIR_NAME),
            None => PathBuf::from("./cache"),
        }
    }
}

impl Default for ContextState {
    fn default() -> Self {
        Self {
            settings: ContextSettings {
                server_uri: Uri::default(),
                request_timeout: Duration::from_secs(30),
                connect_timeout: Duration::from_secs(60),
            },
            last_sync: None,
        }
    }
}

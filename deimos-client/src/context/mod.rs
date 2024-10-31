use std::{path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use deimosproto::DeimosServiceClient;
use futures::{Stream, StreamExt};
use http::Uri;
use pod::CachedPod;
use slotmap::SlotMap;
use tokio::sync::Mutex;
use tonic::transport::{Channel, ClientTlsConfig};

mod load;
pub mod pod;

slotmap::new_key_type! {
    pub struct PodRef;
}

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// Context state preserved in save files
    pub state: ContextState,
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub pods: SlotMap<PodRef, CachedPod>,
    /// Directory that all container data and context state will be saved to
    cache_dir: PathBuf,
    /// State of API connection, determined from gRPC status codes when operations fail
    conn: ContextConnectionState,
    /// Client for the gRPC API
    api: Arc<Mutex<DeimosServiceClient<Channel>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextConnectionState {
    Unknown,
    Connected,
    Error,
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

    /// Save all context state and new data received for containers to the local cache directory
    pub fn save(&self) {
        self.save_state();
        self.save_cached_pods();
    }

    async fn subscribe_pod_events(
        api: Arc<Mutex<DeimosServiceClient<Channel>>>,
    ) -> impl Stream<Item = Result<deimosproto::PodStatusNotification, tonic::Status>> {
        loop {
            let result = api
                .lock()
                .await
                .subscribe_pod_status(deimosproto::PodStatusStreamRequest {})
                .await;
            match result {
                Ok(stream) => break stream.into_inner(),
                Err(e) => {
                    tracing::error!("Failed to subscribe to pod status notifications: {e}");
                }
            }
        }
    }

    /// Load all context state from the local cache directory and begin connection attempts to the
    /// gRPC server with the loaded settings
    pub async fn load() -> Self {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir.join(Self::CACHE_DIR_NAME),
            None => PathBuf::from("./deimos-cache"),
        };

        let state = match Self::load_state(&cache_dir) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!("Failed to load application state: {e}");
                ContextState::default()
            }
        };
        
        let pods = Self::load_cached_pods(cache_dir.clone()).await;
        let api = Arc::new(Mutex::new(Self::connect_api(state.settings.clone()).await));
        let conn = ContextConnectionState::Unknown;
        let state = state;

        Self {
            state,
            api,
            conn,
            pods,
            cache_dir,
        }
    }


    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: ContextSettings) -> DeimosServiceClient<Channel> {
        let channel = Channel::builder(settings.server_uri.clone())
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .unwrap()
            .connect_timeout(settings.connect_timeout)
            .timeout(settings.request_timeout)
            .connect_lazy();

        DeimosServiceClient::new(channel)
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

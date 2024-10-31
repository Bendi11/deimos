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
    api: Option<Arc<Mutex<DeimosServiceClient<Channel>>>>,
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

#[derive(Clone, Debug)]
pub enum ContextMessage {
    /// Loaded all container data from the local cache
    Loaded(Vec<CachedPod>),
    /// gRPC client must be initialized in async context
    ClientInit(Box<DeimosServiceClient<Channel>>),
    /// Upate from API methods informing new connection state
    ConnectionStatus(ContextConnectionState),
    /// Received a listing of containers from the server, so update our local cache to match.
    /// Also remove any containers if their IDs are not contained in the given response
    BeginSynchronizeFromQuery(deimosproto::QueryPodsResponse),
    /// Received all container data including images, so update our in memory data
    SynchronizePod(Box<CachedPod>),
    /// Notification received from the server
    Notification(deimosproto::PodStatusNotification),
    /// An error occured in a future
    Error,
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

    async fn subscribe_pod_logs(
        api: Arc<Mutex<DeimosServiceClient<Channel>>>,
        id: String,
    ) -> Option<impl Stream<Item = Vec<u8>>> {
        match api.lock().await.subscribe_pod_logs(deimosproto::PodLogStreamRequest { id: id.clone() }).await {
            Ok(stream) => Some(
                stream.into_inner().filter_map(|chunk| async move { chunk.ok().map(|chunk| chunk.chunk) })
            ),
            Err(e) => {
                tracing::error!("Failed to subscribe to pod {} logs: {}", id, e);
                None
            }
        }
    }

    /// Subscribe to container status updates from the server
    fn subscription(
        api: Arc<Mutex<DeimosServiceClient<Channel>>>,
    ) -> impl Stream<Item = ContextMessage> {
        async_stream::stream! {
            loop {
                let mut stream = {
                    let mut api = api.lock().await;
                    match api.subscribe_pod_status(deimosproto::PodStatusStreamRequest {}).await {
                        Ok(stream) => {
                            yield ContextMessage::ConnectionStatus(ContextConnectionState::Connected);
                            stream.into_inner()
                        },
                        Err(e) => match e.code() {
                            tonic::Code::Unavailable => {
                                yield ContextMessage::ConnectionStatus(ContextConnectionState::Error);
                                continue
                            },
                            _ => {
                                tracing::error!("Failed to subscribe to pod status stream: {e}");
                                continue
                            }
                        }
                    }
                };

                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            yield ContextMessage::Notification(event)
                        },
                        Err(e) => {
                            tracing::error!("Failed to read pod status notification: {e}");
                        },
                    }
                }

                tracing::warn!("Container status notification stream closed, attempting to reopen");
            }
        }
    }

    /// Load all context state from the local cache directory and begin connection attempts to the
    /// gRPC server with the loaded settings
    pub fn load() -> Self {
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
        
        let conn = ContextConnectionState::Unknown;
        let api = None;
        let state = state;
        let containers = SlotMap::<PodRef, CachedPod>::default();

        Self {
            state,
            api,
            conn,
            pods: containers,
            cache_dir,
        }
    }
    
    fn get_pod_ref(&self, id: &str) -> Option<PodRef> {
        self.pods
            .iter()
            .find_map(|(k, v)| (v.data.id == id).then_some(k))
    }

    fn get_pod(&self, id: &str) -> Option<&CachedPod> {
        self.get_pod_ref(id).and_then(|r| self.pods.get(r))
    }

    fn get_pod_mut(&mut self, id: &str) -> Option<&mut CachedPod> {
        self.get_pod_ref(id).and_then(|r| self.pods.get_mut(r))
    }


    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: ContextSettings) -> Box<DeimosServiceClient<Channel>> {
        let channel = Channel::builder(settings.server_uri.clone())
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .unwrap()
            .connect_timeout(settings.connect_timeout)
            .timeout(settings.request_timeout)
            .connect_lazy();

        Box::new(DeimosServiceClient::new(channel))
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

use std::{borrow::Borrow, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use deimosproto::DeimosServiceClient;
use futures::StreamExt;
use http::Uri;
use im::HashMap;
use pod::{CachedPod, CachedPodData, CachedPodState};
use tokio::sync::Mutex;
use tonic::transport::{Channel, ClientTlsConfig};

mod load;
pub mod pod;

slotmap::new_key_type! {
    pub struct PodRef;
}

#[derive(Debug, Clone)]
pub struct NotifyMutation<T>(tokio::sync::watch::Sender<T>);

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// Context state preserved in save files
    pub state: ContextState,
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub pods: NotifyMutation<HashMap<String, Arc<CachedPod>>>,
    /// Directory that all container data and context state will be saved to
    cache_dir: PathBuf,
    /// State of API connection, determined from gRPC status codes when operations fail
    conn: NotifyMutation<ContextConnectionState>,
    /// Client for the gRPC API
    api: Mutex<Option<DeimosServiceClient<Channel>>>,
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
    pub certificate_path: PathBuf,
    pub privkey_path: PathBuf,
}

/// Persistent state kept for the [Context]'s connection
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextState {
    pub settings: NotifyMutation<ContextSettings>,
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
    
    /// Wait for pod status notifications and update the local cache with their statuses as
    /// required
    pub async fn pod_event_loop(&self) -> ! {
        loop {
            let stream = {
                let Some(ref mut api) = *self.api.lock().await else {
                    let timeout = {
                        let r = self.state.settings.read();
                        r.connect_timeout
                    };
                    tokio::time::sleep(timeout).await;
                    continue
                };
                api.subscribe_pod_status(deimosproto::PodStatusStreamRequest {}).await
            };

            let mut stream = match stream {
                Ok(stream) => stream.into_inner(),
                Err(e) => {
                    if e.code() != tonic::Code::DeadlineExceeded {
                        let timeout = {
                            let settings = self.state.settings.read();
                            settings.connect_timeout
                        };

                        tokio::time::sleep(timeout).await;
                    }
                    tracing::warn!("Failed to subscribe to pod status stream: {}", e);
                    continue
                }
            };
            
            while let Some(event) = stream.next().await {
                let event = match event {
                    Ok(ev) => ev,
                    Err(e) => {
                        tracing::warn!("Error when receiving pod status stream: {}", e);
                        break
                    }
                };

                let pod_state = {
                    let read = self.pods.read();
                    read.get(&event.id).map(|pod| pod.data.up.clone())
                };

                match pod_state {
                    Some(up) => {
                        tracing::trace!("Got pod status notification for {} - {:?}", event.id, event.state());
                        up.set(CachedPodState::from(event.state())).await;
                    },
                    None => {
                        tracing::warn!("Got pod status notification for unknown container {}", event.id);
                    }
                }
            }
        }
    }
    
    /// Attempt to update the status of the given pod
    pub async fn update(&self, pod: &CachedPod, up: CachedPodState) {
        let Some(ref mut api) = *self.api.lock().await else { return };
        
        let request = deimosproto::UpdatePodRequest {
            id: pod.data.id.clone(),
            method: deimosproto::PodState::from(up) as i32,
        };

        match api.update_pod(request).await {
            Ok(_) => {
                tracing::trace!("Successfully updated pod {} state to {:?}", pod.data.id, up);
            },
            Err(e) => {
                tracing::error!("Failed to update pod {} state: {}", pod.data.id, e);
            }
        }
    }
    
    /// Query the server for a list of containers and update our local cache in response
    pub async fn synchronize(&self) {
        let Some(ref mut api) = *self.api.lock().await else { return };
        let brief = match api.query_pods(deimosproto::QueryPodsRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(e) => {
                tracing::error!("Failed to query pods from server: {}", e);
                return
            }
        };

        let mut pods = self.pods.read().clone();
        pods.retain(|id, _| brief.pods.iter().any(|recv| recv.id == *id));

        for pod in brief.pods {
            match pods.get_mut(&pod.id) {
                Some(exist) => {
                    exist.data.up.set(CachedPodState::from(pod.state())).await;
                    //exist.data.name = pod.title;
                },
                None => {
                    tracing::trace!("Received new pod {} from server", pod.id);
                    let data = CachedPodData {
                        up: NotifyMutation::new(CachedPodState::from(pod.state())),
                        id: pod.id,
                        name: pod.title,
                    };

                    let pod = CachedPod {
                        data,
                    };

                    pods.insert(pod.data.id.clone(), Arc::new(pod));
                }
            }
        }

        self.pods.set(pods).await;
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
        
        let pods = NotifyMutation::new(HashMap::default());
        let api = Mutex::new(Self::connect_api(state.settings.read().clone()).await);
        let conn = NotifyMutation::new(ContextConnectionState::Unknown);

        Self {
            state,
            api,
            conn,
            pods,
            cache_dir,
        }
    }

    pub async fn init(&self) {
        self.pods.set(Self::load_cached_pods(self.cache_dir.clone()).await).await;
        self.state.settings.notify().await;
    }
    
    /// Reload the API connection using the given context settings
    pub async fn reload(&self, settings: ContextSettings) {
        let (_, mut api) = tokio::join!(self.state.settings.set(settings.clone()), self.api.lock());
        *api = Self::connect_api(settings).await;
    }


    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: ContextSettings) -> Option<DeimosServiceClient<Channel>> {
        let channel = Channel::builder(settings.server_uri.clone())
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .ok()?
            .connect_timeout(settings.connect_timeout)
            .timeout(settings.request_timeout)
            .connect_lazy();

        Some(
            DeimosServiceClient::new(channel)
        )
    }
}

impl Default for ContextState {
    fn default() -> Self {
        Self {
            settings: NotifyMutation::new(ContextSettings::default()),
            last_sync: None,
        }
    }
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
            certificate_path: PathBuf::from("./cert.pem"),
            privkey_path: PathBuf::from("./key.pem"),
        }
    }
}

impl<T> NotifyMutation<T> {
    /// Create a new wrapper that will notify UI elements of mutations to the given value
    pub fn new(val: T) -> Self {
        let (tx, _) = tokio::sync::watch::channel(val);
        tx.send_modify(|_| ());
        Self(tx)
    }
    
    /// Get a receiver that will notify tasks when the given value is mutated
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<T> {
        self.0.subscribe()
    }
    
    /// Get the current value
    pub fn read(&self) -> tokio::sync::watch::Ref<T> {
        self.0.borrow()
    }
    
    /// Set the current value, notifying all waiting tasks of a mutation
    pub async fn set(&self, val: T) {
        let _ = self.0.send_replace(val);
    }
    
    /// Notify waiting subscribers without modifying the contained value
    pub async fn notify(&self) {
        self.0.send_modify(|_| ());
    }
}

impl<T: serde::Serialize> serde::Serialize for NotifyMutation<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        self.0.borrow().serialize(serializer)
    }
}

impl<'a, T: serde::Deserialize<'a>> serde::Deserialize<'a> for NotifyMutation<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'a> {
        T::deserialize(deserializer)
            .map(|val| Self(tokio::sync::watch::channel(val).0))
    }
}

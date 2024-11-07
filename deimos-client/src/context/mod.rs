use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::StreamExt;
use layer::{cancel::{CancelLayer, CancelService}, conn::{ConnectionTracker, ConnectionTrackerLayer}};
use deimosproto::DeimosServiceClient;
use http::Uri;
use im::HashMap;
use pod::{CachedPod, CachedPodData, CachedPodState};
use tokio::sync::{Mutex, Notify};
use tonic::{metadata::MetadataValue, service::Interceptor, transport::{Channel, ClientTlsConfig}};
use zeroize::Zeroizing;

mod auth;
mod load;
mod layer;
pub mod pod;

#[derive(Debug, Clone)]
pub struct NotifyMutation<T>(tokio::sync::watch::Sender<T>);

#[derive(Clone)]
struct AuthenticationInterceptor(Option<Zeroizing<Vec<u8>>>);

type ApiClient = DeimosServiceClient<
    tonic::service::interceptor::InterceptedService<
        CancelService<ConnectionTracker<Channel>>,
        AuthenticationInterceptor
    >
>;

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// Context state preserved in save files
    pub persistent: ContextPersistent,
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub pods: NotifyMutation<HashMap<String, Arc<CachedPod>>>,
    /// Directory that all container data and context state will be saved to
    cache_dir: PathBuf,
    /// State of API connection, determined from gRPC status codes when operations fail
    pub conn: NotifyMutation<ContextConnectionState>,
    /// Client for the gRPC API
    api: Mutex<Option<ApiClient>>,
    /// Notifier that will cancel any pending API requests
    cancel: Arc<Notify>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextConnectionState {
    Unknown,
    NoToken,
    Connected,
    Error,
}

/// Persistent state kept for the [Context]'s connection
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextPersistent {
    pub settings: NotifyMutation<ContextSettings>,
    pub token: NotifyMutation<Option<PersistentToken>>,
}

/// Settings that may be adjusted by the user
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextSettings {
    #[serde(with = "http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
}

/// A token stored in the context save file - this may be encrypted with platform-specific APIs
/// and may need to be decrypted before use with an [AuthenticationInterceptor]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PersistentToken {
    kind: PersistentTokenKind,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub enum PersistentTokenKind {
    Plaintext,
    #[cfg(windows)]
    Dpapi,
}

impl PersistentToken {
    pub fn protect(kind: PersistentTokenKind, data: Vec<u8>) -> Result<Self, String> {
        match kind {
            PersistentTokenKind::Plaintext => Ok(
                Self {
                    kind,
                    data,
                }
            ),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => Ok(
                Self {
                    kind,
                    data: auth::dpapi::unprotect(&data).map_err(|e| e.to_string()),      
                }
            ),
        }
    }

    pub fn unprotect(&self) -> Result<Zeroizing<Vec<u8>>, String>  {
        match self.kind {
            PersistentTokenKind::Plaintext => Ok(self.data.clone().into()),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => auth::dpapi::protect(&self.data).map(Into::into).map_err(|e| e.to_string()),
        }
    }
}

impl Drop for PersistentToken {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.data.zeroize();
    }
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
                let mut api = self.api.lock().await;
                let Some(ref mut api) = *api else {
                    let timeout = {
                        let r = self.persistent.settings.read();
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
                            let settings = self.persistent.settings.read();
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
                        up.set(CachedPodState::from(event.state()));
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
                tracing::warn!("Failed to update pod {} state: {}", pod.data.id, e);
            }
        }
    }
    
    /// Query the server for a list of containers and update our local cache in response
    pub async fn synchronize(&self) {
        let Some(ref mut api) = *self.api.lock().await else { return };
        let brief = match api.query_pods(deimosproto::QueryPodsRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(e) => {
                tracing::warn!("Failed to query pods from server: {}", e);
                return
            }
        };

        let mut pods = self.pods.read().clone();
        pods.retain(|id, _| brief.pods.iter().any(|recv| recv.id == *id));

        for pod in brief.pods {
            match pods.get_mut(&pod.id) {
                Some(exist) => {
                    exist.data.up.set(CachedPodState::from(pod.state()));
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

        self.pods.set(pods);
    }

    /// Load all context state from the local cache directory and begin connection attempts to the
    /// gRPC server with the loaded settings
    pub async fn load() -> Self {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir.join(Self::CACHE_DIR_NAME),
            None => PathBuf::from("./deimos-cache"),
        };

        let persistent = match Self::load_state(&cache_dir) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!("Failed to load application state: {e}");
                ContextPersistent::default()
            }
        };
        
        let pods = NotifyMutation::new(HashMap::default());
        let api = Mutex::new(None);
        let conn = NotifyMutation::new(ContextConnectionState::Unknown);
        let cancel = Arc::new(Notify::new());

        let this = Self {
            persistent,
            api,
            conn,
            pods,
            cache_dir,
            cancel,
        };

        this.connect_api().await;

        this
    }

    pub async fn init(&self) {
        self.pods.set(Self::load_cached_pods(self.cache_dir.clone()).await);
        self.persistent.settings.notify();
    }
    
    /// Reload the API connection using the given context settings
    pub async fn reload(&self, settings: ContextSettings) {
        self.persistent.settings.set(settings.clone());
        self.connect_api().await;
    }


    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(&self) {
        let token = self.persistent.token.read().clone();
        let token = match token {
            Some(ref token) => match token.unprotect() {
                Ok(unprotect) => Some(unprotect),
                Err(e) => {
                    tracing::error!("Failed to unprotect token: {}", e);
                    return
                }
            },
            None => None,
        };
        
        let channel = {
            let settings = self.persistent.settings.read();
            Channel::builder(settings.server_uri.clone())
                .connect_timeout(settings.connect_timeout)
                .timeout(settings.request_timeout)
                .tls_config(ClientTlsConfig::new().with_webpki_roots())
                .ok()
        };

        let Some(channel) = channel else { return };

        self.cancel.notify_waiters();
        let mut lock = self.api.lock().await;

        
        let channel = channel.connect_lazy();
        let client = DeimosServiceClient::with_interceptor(
            tower::ServiceBuilder::new()
                .layer(CancelLayer::new(self.cancel.clone()))
                .layer(ConnectionTrackerLayer::new(self.conn.clone()))
                .service(channel),
            AuthenticationInterceptor(token)
        );
        
        *lock = Some(client);
    }
}

impl Interceptor for AuthenticationInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(ref token) = self.0 {
            request
                .metadata_mut()
                .insert_bin(
                    "authorization-bin",
                    MetadataValue::from_bytes(token)
                );
        }

        Ok(request)
    }
}

impl Default for ContextPersistent {
    fn default() -> Self {
        Self {
            settings: NotifyMutation::new(ContextSettings::default()),
            token: NotifyMutation::new(None),
        }
    }
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
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
    pub fn set(&self, val: T) {
        let _ = self.0.send_replace(val);
    }
    
    /// Notify waiting subscribers without modifying the contained value
    pub fn notify(&self) {
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

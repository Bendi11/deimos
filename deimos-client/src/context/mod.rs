use std::{path::PathBuf, sync::Arc};

use client::{ContextClients, ContextPersistent};
use futures::StreamExt;
use im::HashMap;
use pod::{CachedPod, CachedPodData, CachedPodState};

mod load;
pub mod client;
pub mod pod;

#[derive(Debug, Clone, Default)]
pub struct NotifyMutation<T>(tokio::sync::watch::Sender<T>);

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub pods: NotifyMutation<HashMap<String, Arc<CachedPod>>>,
    /// Pod control and authorization API clients
    pub clients: ContextClients,
    /// Directory that all container data and context state will be saved to
    cache_dir: PathBuf,
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
                let mut api = self.clients.podapi().await;
                let Some(ref mut api) = api else {
                    let timeout = {
                        let r = self.clients.persistent.settings.read();
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
                            let settings = self.clients.persistent.settings.read();
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
        let Some(ref mut api) = self.clients.podapi().await else { return };
        
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
        let Some(ref mut api) = self.clients.podapi().await else { return };
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
        let clients = ContextClients::new(persistent).await;

        Self {
            pods,
            clients,
            cache_dir,
        }
    }

    pub async fn init(&self) {
        self.pods.set(Self::load_cached_pods(self.cache_dir.clone()).await);
        self.clients.persistent.settings.notify();
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

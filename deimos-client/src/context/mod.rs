use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use container::{CachedContainer, CachedContainerRunKind, CachedContainerRunState};
use deimos_shared::{ContainerStatusNotification, ContainerStatusStreamRequest, DeimosServiceClient, QueryContainersRequest, QueryContainersResponse, UpdateContainerRequest};
use http::Uri;
use iced::futures::{Stream, StreamExt};
use slotmap::SlotMap;
use tokio::sync::Mutex;
use tonic::transport::Channel;

pub mod container;
mod load;

slotmap::new_key_type! {
    pub struct ContainerRef;
}

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// Context state preserved in save files
    pub state: ContextState,
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub containers: SlotMap<ContainerRef, CachedContainer>,
    /// Directory that all container data and context state will be saved to
    cache_dir: PathBuf,
    api: Arc<Mutex<DeimosServiceClient<Channel>>>,
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
    /// Received a listing of containers from the server, so update our local cache to match.
    /// Also remove any containers if their IDs are not contained in the given response
    BeginSynchronizeFromQuery(QueryContainersResponse),
    /// Received all container data including images, so update our in memory data
    SynchronizeContainer(CachedContainer),
    /// Notification received from the server
    Notification(ContainerStatusNotification),
    /// An error occured in a future
    Error,
}

impl Context {
    pub const CACHE_DIR_NAME: &str = "deimos";
    
    /// Save all context state and new data received for containers to the local cache directory
    pub fn save(&self) {
        self.save_state();
        self.save_cached_containers();
    }
    
    /// Subscribe to container status updates from the server
    pub async fn subscription(&self) -> impl Stream<Item = ContextMessage> {
        let result = self.api.lock().await.container_status_stream(ContainerStatusStreamRequest {}).await;
        result
            .unwrap()
            .into_inner()
            .filter_map(|s| async move {
                match s {
                    Ok(s) => Some(ContextMessage::Notification(s)),
                    Err(e) => {
                        tracing::error!("Failed to receive container status notification from server: {e}");
                        None
                    }
                }
            })
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

        let api = Arc::new(Mutex::new(Self::connect_api(&state.settings).await));
        let state = state;
        let containers = SlotMap::<ContainerRef, CachedContainer>::default();

        let mut me = Self {
            state,
            api,
            containers,
            cache_dir
        };

        me.load_cached_containers().await;

        me
    }

    fn get_container_ref(&self, id: &str) -> Option<ContainerRef> {
        self
            .containers
            .iter()
            .find_map(|(k, v)| (v.data.id == id).then_some(k))
    }

    fn get_container(&self, id: &str) -> Option<&CachedContainer> {
        self.get_container_ref(id).and_then(|r| self.containers.get(r))
    }

    fn get_container_mut(&mut self, id: &str) -> Option<&mut CachedContainer> {
        self.get_container_ref(id).and_then(|r| self.containers.get_mut(r))
    }

    pub fn update(&mut self, msg: ContextMessage) -> iced::Task<ContextMessage> {
        match msg {
            ContextMessage::BeginSynchronizeFromQuery(resp) => self.begin_synchronize_from_query(resp),
            ContextMessage::SynchronizeContainer(container) => {
                match self.get_container_ref(&container.data.id) {
                    Some(exist) => {
                        self.containers[exist] = container;
                    },
                    None => {
                        self.containers.insert(container);
                    }
                }

                iced::Task::none()
            },
            ContextMessage::Notification(notify) => match self.get_container_mut(&notify.container_id) {
                Some(container) => {
                    tracing::trace!("Got status notification for {}", notify.container_id);
                    container.data.running = notify.status.and_then(Self::docker_status_response_to_state);
                    iced::Task::none()
                },
                None => {
                    tracing::error!("Got status notification for unknown container '{}', attempting synchronization", notify.container_id);
                    self.synchronize_from_server()
                }
            }
            ContextMessage::Error => iced::Task::none(),
        }
    }
    
    /// Synchronize all container data from the server and update our local cache
    pub fn synchronize_from_server(&self) -> iced::Task<ContextMessage> {
        let api = self.api.clone();
        iced::Task::future(
            async move {
                let mut api = api.lock().await;
                match api.query_containers(QueryContainersRequest {}).await {
                    Ok(resp) => ContextMessage::BeginSynchronizeFromQuery(resp.into_inner()),
                    Err(e) => {
                        tracing::error!("Failed to query containers from server: {e}");
                        ContextMessage::Error
                    }
                }
            }
        )
    }
    
    /// Change the given container's status on the server
    pub fn update_container(&self, container: ContainerRef, on: bool) -> iced::Task<ContextMessage> {
        let Some(id) = self.containers.get(container).map(|c| c.data.id.clone()) else {
            tracing::warn!("Got update container message for unknown container '{:?}'", container);
            return iced::Task::none()
        };

        let api = self.api.clone();
        iced::Task::future(
            async move {
                let mut api = api.lock().await;
                let method = if on { deimos_shared::UpdateContainerMethod::Start } else { deimos_shared::UpdateContainerMethod::Stop } as i32;
                if let Err(e) = api.update_container(UpdateContainerRequest { id: id.clone(), method }).await {
                    tracing::error!("Failed to update container {}: {}", id, e);
                }
            }
        ).discard()
    }
    
    /// Start synchronizing the local cache items from the given list of containers on the server
    fn begin_synchronize_from_query(&mut self, resp: QueryContainersResponse) -> iced::Task<ContextMessage> {
        self
            .containers
            .retain(|_, exist| {
                let present = resp.containers.iter().any(|c| c.id == exist.data.id);
                if !present {
                    tracing::trace!("Removing container {} - was not contained in server's response", exist.data.id);
                }

                present
            });

        let tasks = resp
            .containers
            .into_iter()
            .filter_map(|new| 
                match DateTime::<Utc>::from_timestamp(new.updated, 0) {
                    Some(updated) => match self.get_container_ref(&new.id).and_then(|r| self.containers.get(r)) {
                        Some(local) => match updated > local.data.last_update {
                            true => {
                                tracing::trace!("Updating container {} with newer version from remote", local.data.id);
                                true
                            },
                            false => {
                                tracing::trace!("Not updating container {} - have newest version", local.data.id);
                                false
                            }
                        },
                        None => {
                            tracing::info!("Got new container {} from server", new.id);
                            true
                        }
                    }.then(||
                        iced::Task::perform(
                            Self::synchronize_container_from_brief(self.api.clone(), new),
                            ContextMessage::SynchronizeContainer
                        )
                    ),
                    None => {
                        tracing::error!("Failed to decode last updated timestamp {} for {}", new.updated, new.id);
                        None
                    }
                }
            );

        iced::Task::batch(tasks)
    }
    
    /// Restart the client, applying any connection parameter changes since the last connection
    pub fn reload_settings(&self) -> iced::Task<ContextMessage> {
        let api = self.api.clone();
        let settings = self.state.settings.clone();
        iced::Task::future(async move {
            let mut api = api.lock().await;
            *api = Self::connect_api(&settings).await;
        }).discard()
    }
    
    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: &ContextSettings) -> DeimosServiceClient<Channel> {
        DeimosServiceClient::new(
            Channel::builder(settings.server_uri.clone())
                .connect_timeout(settings.connect_timeout)
                .timeout(settings.request_timeout)
                .connect_lazy(),
        )
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

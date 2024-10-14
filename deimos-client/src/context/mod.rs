use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use container::{CachedContainer, CachedContainerData, CachedContainerUpState};
use deimosproto::DeimosServiceClient;
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
    api: Option<Arc<Mutex<DeimosServiceClient<Channel>>>>,
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
    Loaded(Vec<CachedContainer>),
    Connected(DeimosServiceClient<Channel>),
    /// Received a listing of containers from the server, so update our local cache to match.
    /// Also remove any containers if their IDs are not contained in the given response
    BeginSynchronizeFromQuery(deimosproto::QueryContainersResponse),
    /// Received all container data including images, so update our in memory data
    SynchronizeContainer(CachedContainer),
    /// Notification received from the server
    Notification(deimosproto::ContainerStatusNotification),
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

    async fn subscribe_container_events(api: Arc<Mutex<DeimosServiceClient<Channel>>>) -> impl Stream<Item = Result<deimosproto::ContainerStatusNotification, tonic::Status>> {
        loop {
            let result = api.lock().await.container_status_stream(deimosproto::ContainerStatusStreamRequest {}).await;
            match result {
                Ok(stream) => break stream.into_inner(),
                Err(e) => {
                    tracing::error!("Failed to subscribe to container status notifications: {e}");
                }
            }
        }
    }
    
    /// Subscribe to container status updates from the server
    fn subscription(api: Arc<Mutex<DeimosServiceClient<Channel>>>) -> impl Stream<Item = ContextMessage> {
        async_stream::stream! {
            loop {
                let mut stream = Self::subscribe_container_events(api.clone()).await;
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            yield ContextMessage::Notification(event)
                        },
                        Err(e) => {
                            tracing::error!("Failed to read container status notification: {e}");
                        }
                    }
                }

                tracing::warn!("Container status notification stream closed, attempting to reopen");
            }
        }
    }
    
    /// Start a task to process container status notifications
    fn container_notification_task(api: Arc<Mutex<DeimosServiceClient<Channel>>>) -> iced::Task<ContextMessage> {
        iced::Task::stream(Self::subscription(api))
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

        let api = None;
        let state = state;
        let containers = SlotMap::<ContainerRef, CachedContainer>::default();

        Self {
            state,
            api,
            containers,
            cache_dir
        }
    }

    pub fn post_load_init(&self) -> iced::Task<ContextMessage> {
        iced::Task::perform(
            Self::load_cached_containers(self.cache_dir.clone()),
            ContextMessage::Loaded,
        ).chain(
            iced::Task::perform(
                Self::connect_api(self.state.settings.clone()),
                ContextMessage::Connected
            )
        )
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
            ContextMessage::Loaded(containers) => {
                for c in containers {
                    self.containers.insert(c);
                }
                iced::Task::none()
            },
            ContextMessage::Connected(api) => {
                let api = Arc::new(Mutex::new(api));
                self.api = Some(api.clone());
                Self::container_notification_task(api)
            },
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
                    container.data.up = match deimosproto::ContainerUpState::try_from(notify.up_state).map(CachedContainerUpState::from) {
                        Ok(up) => up,
                        Err(_) => {
                            tracing::error!("Unknown up status {} received for container '{}'", notify.up_state, notify.container_id);
                            return iced::Task::none()
                        }
                    };
                    iced::Task::none()
                },
                None => {
                    tracing::error!("Got status notification for unknown container '{}', attempting synchronization", notify.container_id);
                    self.synchronize_from_server()
                }
            },
            ContextMessage::Error => iced::Task::none(),
        }
    }
    
    /// Synchronize all container data from the server and update our local cache
    pub fn synchronize_from_server(&self) -> iced::Task<ContextMessage> {
        let Some(api) = self.api.clone() else { return iced::Task::none() };
        iced::Task::future(
            async move {
                let mut api = api.lock().await;
                match api.query_containers(deimosproto::QueryContainersRequest {}).await {
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
    pub fn update_container(&self, container: ContainerRef, run: CachedContainerUpState) -> iced::Task<ContextMessage> {
        let Some(id) = self.containers.get(container).map(|c| c.data.id.clone()) else {
            tracing::warn!("Got update container message for unknown container '{:?}'", container);
            return iced::Task::none()
        };

        let Some(api) = self.api.clone() else { return iced::Task::none() };
        iced::Task::future(
            async move {
                let mut api = api.lock().await;
                let method: deimosproto::ContainerUpState = run.into();
                if let Err(e) = api.update_container(deimosproto::UpdateContainerRequest { id: id.clone(), method: method as i32 }).await {
                    tracing::error!("Failed to update container {}: {}", id, e);
                }
            }
        ).discard()
    }
    
    /// Start synchronizing the local cache items from the given list of containers on the server
    fn begin_synchronize_from_query(&mut self, resp: deimosproto::QueryContainersResponse) -> iced::Task<ContextMessage> {
        let Some(api) = self.api.clone() else { return iced::Task::none() };

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
            .filter_map(|new| {
                let up  = match deimosproto::ContainerUpState::try_from(new.up_state).map(CachedContainerUpState::from) {
                    Ok(up) => up,
                    Err(_) => {
                        tracing::error!("Failed to decode up status of container '{}', defaulting to dead", new.id);
                        CachedContainerUpState::Dead
                    }
                };

                let data = CachedContainerData {
                    id: new.id,
                    name: new.title,
                    up,
                };

                match self.get_container_mut(&data.id) {
                    Some(local) => {
                        local.data = data;
                        None
                    },
                    None => {
                        tracing::info!("Got new container {} from server", data.id);
                        self.containers.insert(CachedContainer {
                            data,
                            banner: None,
                            icon: None
                        });
                        None
                    }
                }
            }
        );

        iced::Task::batch(tasks)
    }
    
    /// Restart the client, applying any connection parameter changes since the last connection
    pub fn reload_settings(&self) -> iced::Task<ContextMessage> {
        let Some(api) = self.api.clone() else { return iced::Task::none() };
        let settings = self.state.settings.clone();
        iced::Task::future(async move {
            let mut api = api.lock().await;
            *api = Self::connect_api(settings).await;
        }).discard()
    }
    
    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: ContextSettings) -> DeimosServiceClient<Channel> {
        let channel = loop {
            let channel = Channel::builder(settings.server_uri.clone())
                .connect_timeout(settings.connect_timeout)
                .timeout(settings.request_timeout)
                .connect()
                .await;

            match channel {
                Ok(c) => break c,
                Err(e) => {
                    tracing::error!("Failed to connect to API: {e}");
                    continue
                }
            }
        };

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

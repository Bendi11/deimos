use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use container::CachedContainer;
use deimos_shared::{DeimosServiceClient, QueryContainersRequest, QueryContainersResponse};
use http::Uri;
use tokio::sync::Mutex;
use tonic::transport::Channel;

pub mod container;
mod load;

/// Context shared across the application used to perform API requests and maintain a local
/// container cache.
#[derive(Debug)]
pub struct Context {
    /// Context state preserved in save files
    pub state: ContextState,
    /// A map of all loaded containers, to be modified by gRPC notifications
    pub containers: HashMap<String, CachedContainer>,
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
        let containers = HashMap::new();

        let mut me = Self {
            state,
            api,
            containers,
            cache_dir
        };

        me.load_cached_containers().await;

        me
    }

    pub fn update(&mut self, msg: ContextMessage) -> iced::Task<ContextMessage> {
        match msg {
            ContextMessage::BeginSynchronizeFromQuery(resp) => self.begin_synchronize_from_query(resp),
            ContextMessage::SynchronizeContainer(container) => {
                self.containers.insert(container.data.id.clone(), container);
                iced::Task::none()
            },
            ContextMessage::Error => iced::Task::none(),
        }
    }

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
    
    /// Start synchronizing the local cache items from the given list of containers on the server
    fn begin_synchronize_from_query(&mut self, resp: QueryContainersResponse) -> iced::Task<ContextMessage> {
        self
            .containers
            .retain(|id, _| {
                let present = resp.containers.iter().any(|c| c.id == *id);
                if !present {
                    tracing::trace!("Removing container {} - was not contained in server's response", id);
                }

                present
            });

        let tasks = resp
            .containers
            .into_iter()
            .filter_map(|new| 
                match DateTime::<Utc>::from_timestamp(new.updated, 0) {
                    Some(updated) => match self.containers.get(&new.id) {
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

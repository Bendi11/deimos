use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use pod::{
    CachedPod, CachedPodData, CachedPodState, CachedPodStateFull,
};
use deimosproto::DeimosServiceClient;
use http::Uri;
use iced::futures::{Stream, StreamExt};
use slotmap::SlotMap;
use tokio::sync::Mutex;
use tonic::transport::Channel;

pub mod pod;
mod load;

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
    /// Loaded all container data from the local cache
    Loaded(Vec<CachedPod>),
    /// gRPC client must be initialized in async context
    ClientInit(Box<DeimosServiceClient<Channel>>),
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

    /// Subscribe to container status updates from the server
    fn subscription(
        api: Arc<Mutex<DeimosServiceClient<Channel>>>,
    ) -> impl Stream<Item = ContextMessage> {
        async_stream::stream! {
            loop {
                let mut stream = Self::subscribe_pod_events(api.clone()).await;
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            yield ContextMessage::Notification(event)
                        },
                        Err(e) => {
                            tracing::error!("Failed to read pod status notification: {e}");
                        }
                    }
                }

                tracing::warn!("Container status notification stream closed, attempting to reopen");
            }
        }
    }

    /// Start a task to process container status notifications
    fn pod_notification_task(
        api: Arc<Mutex<DeimosServiceClient<Channel>>>,
    ) -> iced::Task<ContextMessage> {
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
        let containers = SlotMap::<PodRef, CachedPod>::default();

        Self {
            state,
            api,
            pods: containers,
            cache_dir,
        }
    }

    pub fn post_load_init(&self) -> iced::Task<ContextMessage> {
        iced::Task::perform(
            Self::load_cached_pods(self.cache_dir.clone()),
            ContextMessage::Loaded,
        )
        .chain(iced::Task::perform(
            Self::connect_api(self.state.settings.clone()),
            ContextMessage::ClientInit,
        ))
    }

    fn get_pod_ref(&self, id: &str) -> Option<PodRef> {
        self.pods
            .iter()
            .find_map(|(k, v)| (v.data.id == id).then_some(k))
    }

    fn get_pod(&self, id: &str) -> Option<&CachedPod> {
        self.get_pod_ref(id)
            .and_then(|r| self.pods.get(r))
    }

    fn get_pod_mut(&mut self, id: &str) -> Option<&mut CachedPod> {
        self.get_pod_ref(id)
            .and_then(|r| self.pods.get_mut(r))
    }

    pub fn update(&mut self, msg: ContextMessage) -> iced::Task<ContextMessage> {
        match msg {
            ContextMessage::Loaded(pods) => {
                for c in pods {
                    self.pods.insert(c);
                }
                iced::Task::none()
            }
            ContextMessage::ClientInit(api) => {
                let api = Arc::new(Mutex::new(*api));
                self.api = Some(api.clone());
                Self::pod_notification_task(api)
            }
            ContextMessage::BeginSynchronizeFromQuery(resp) => {
                self.begin_synchronize_from_query(resp)
            }
            ContextMessage::SynchronizePod(pod) => {
                match self.get_pod_ref(&pod.data.id) {
                    Some(exist) => {
                        self.pods[exist] = *pod;
                    }
                    None => {
                        self.pods.insert(*pod);
                    }
                }

                iced::Task::none()
            }
            ContextMessage::Notification(notify) => {
                match self.get_pod_mut(&notify.id) {
                    Some(pod) => {
                        tracing::trace!("Got status notification for {}", notify.id);
                        pod.data.up =
                            match deimosproto::PodState::try_from(notify.state)
                                .map(CachedPodState::from)
                            {
                                Ok(up) => up.into(),
                                Err(_) => {
                                    tracing::error!(
                                        "Unknown up status {} received for pod '{}'",
                                        notify.state,
                                        notify.id
                                    );
                                    return iced::Task::none();
                                }
                            };
                        iced::Task::none()
                    }
                    None => {
                        tracing::error!("Got status notification for unknown pod '{}', attempting synchronization", notify.id);
                        self.synchronize_from_server()
                    }
                }
            }
            ContextMessage::Error => iced::Task::none(),
        }
    }

    /// Synchronize all container data from the server and update our local cache
    pub fn synchronize_from_server(&self) -> iced::Task<ContextMessage> {
        let Some(api) = self.api.clone() else {
            return iced::Task::none();
        };
        iced::Task::future(async move {
            let mut api = api.lock().await;
            match api
                .query_pods(deimosproto::QueryPodsRequest {})
                .await
            {
                Ok(resp) => ContextMessage::BeginSynchronizeFromQuery(resp.into_inner()),
                Err(e) => {
                    tracing::error!("Failed to query pods from server: {e}");
                    ContextMessage::Error
                }
            }
        })
    }

    /// Change the given container's status on the server
    pub fn update_pod(
        &mut self,
        pod: PodRef,
        run: CachedPodState,
    ) -> iced::Task<ContextMessage> {
        let id = match self.pods.get_mut(pod) {
            Some(pod) => {
                pod.data.up = CachedPodStateFull::UpdateRequested {
                    old: match pod.data.up {
                        CachedPodStateFull::Known(s) => s,
                        _ => CachedPodState::Disabled,
                    },
                    req: run,
                };

                pod.data.id.clone()
            }
            None => {
                tracing::warn!(
                    "Got update container message for unknown container '{:?}'",
                    pod
                );
                return iced::Task::none();
            }
        };

        let Some(api) = self.api.clone() else {
            return iced::Task::none();
        };
        iced::Task::future(async move {
            let mut api = api.lock().await;
            let method: deimosproto::PodState = run.into();
            if let Err(e) = api
                .update_pod(deimosproto::UpdatePodRequest {
                    id: id.clone(),
                    method: method as i32,
                })
                .await
            {
                tracing::error!("Failed to update pod {}: {}", id, e);
            }
        })
        .discard()
    }

    /// Start synchronizing the local cache items from the given list of containers on the server
    fn begin_synchronize_from_query(
        &mut self,
        resp: deimosproto::QueryPodsResponse,
    ) -> iced::Task<ContextMessage> {
        self.pods.retain(|_, exist| {
            let present = resp.pods.iter().any(|c| c.id == exist.data.id);
            if !present {
                tracing::trace!(
                    "Removing pod {} - was not contained in server's response",
                    exist.data.id
                );
            }

            present
        });

        let tasks = resp.pods.into_iter().filter_map(|new| {
            let up = match deimosproto::PodState::try_from(new.state)
                .map(CachedPodState::from)
            {
                Ok(up) => up,
                Err(_) => {
                    tracing::error!(
                        "Failed to decode up status of pod '{}', defaulting to dead",
                        new.id
                    );
                    CachedPodState::Disabled
                }
            };

            let data = CachedPodData {
                id: new.id,
                name: new.title,
                up: up.into(),
            };

            match self.get_pod_mut(&data.id) {
                Some(local) => {
                    local.data = data;
                    None
                }
                None => {
                    tracing::info!("Got new container {} from server", data.id);
                    self.pods.insert(
                        CachedPod {
                            data,
                        }
                    );
                    None
                }
            }
        });

        iced::Task::batch(tasks)
    }

    /// Restart the client, applying any connection parameter changes since the last connection
    pub fn reload_settings(&self) -> iced::Task<ContextMessage> {
        let Some(api) = self.api.clone() else {
            return iced::Task::none();
        };
        let settings = self.state.settings.clone();
        iced::Task::future(async move {
            let mut api = api.lock().await;
            *api = *Self::connect_api(settings).await;
        })
        .discard()
    }

    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(settings: ContextSettings) -> Box<DeimosServiceClient<Channel>> {
        let channel = Channel::builder(settings.server_uri.clone())
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

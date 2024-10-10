use std::{collections::HashMap, path::PathBuf, sync::Arc};

use bollard::{container::RemoveContainerOptions, secret::{EventMessage, EventMessageTypeEnum}, system::EventsOptions, Docker};
use container::{ManagedContainer, ManagedContainerError, ManagedContainerRunning, ManagedContainerState};
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

pub mod container;

/// Service managing the creation and removal of Docker containers
pub struct DockerState {
    pub config: DockerConfig,
    pub docker: Docker,
    /// Map of managed container IDs to their config and state
    pub containers: DashMap<String, Arc<ManagedContainer>>,
}

/// Configuration for the local Docker container management service
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConfig {
    pub containerdir: PathBuf,
    pub conn: Option<DockerConnectionConfig>,
}

/// Configuration governing how the server will connect to the Docker API
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConnectionConfig {
    pub kind: DockerConnectionType,
    pub addr: String,
    pub timeout: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
pub enum DockerConnectionType {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "local")]
    Local,
}

impl DockerState {
    /// Default timeout to use for Docker API when no alternative is specified in the config
    pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

    /// Load the docker service from a config file specifying how to connect to the local Docker
    /// engine and where to load container configurations from
    pub async fn new(config: DockerConfig) -> Result<Self, DockerInitError> {
        let docker = match config.conn {
            None => {
                tracing::info!("No docker config given, using platform defaults to connect");
                Docker::connect_with_local_defaults()
            }
            Some(ref cfg) => {
                let timeout = cfg.timeout.unwrap_or(Self::DEFAULT_TIMEOUT_SECONDS);
                match cfg.kind {
                    DockerConnectionType::Http => {
                        Docker::connect_with_http(&cfg.addr, timeout, bollard::API_DEFAULT_VERSION)
                    }
                    DockerConnectionType::Local => Docker::connect_with_socket(
                        &cfg.addr,
                        timeout,
                        bollard::API_DEFAULT_VERSION,
                    ),
                }
            }
        }?;

        let containers = DashMap::new();
        
        let dir = config.containerdir.clone();
        let container_entries = std::fs::read_dir(&dir)
            .map_err(|err| DockerInitError::ContainersDirError { dir: dir.clone(), err })?;

        for entry in container_entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    tracing::warn!(
                        "I/O error when reading entries from container config directory {}: {}",
                        dir.display(),
                        e
                    );
                    continue
                }
            };

            let container = match entry.file_type() {
                Ok(fty) if fty.is_dir() => {
                    ManagedContainer::load_from_dir(entry.path(), &docker).await
                },
                Ok(_) => {
                    tracing::warn!(
                        "Unknown file in container config directory: {}",
                        entry.path().display()
                    );
                    continue;
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to get filetype of container config directory entry {}: {}",
                        entry.path().display(),
                        e
                    );
                    continue;
                }
            };

            match container {
                Ok(c) => {
                    let name = c.container_name().to_owned();
                    if containers.insert(name.clone(), Arc::new(c)).is_some() {
                        return Err(DockerInitError::DuplicateConfiguration(name));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load container configuration from {}: {}",
                        entry.path().display(),
                        e
                    );
                }
            }
        }

        if containers.is_empty() {
            tracing::warn!("Deimos server starting with no docker containers configured");
        }

        Ok(Self {
            config,
            docker,
            containers,
        })
    }
    
    /// Create a container from the configuration loaded for the given managed container
    pub async fn create_container(self: &Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let config = managed.docker_config();

        let response = self.docker
            .create_container::<String, String>(
                None,
                config
            )
            .await?;

        tracing::trace!("Created container with ID {} for {}", response.id, managed.container_name());

        for warning in response.warnings {
            tracing::warn!("Warning when creating container {}: {}", managed.container_name(), warning);
        }

        self.docker
            .rename_container(
                &response.id,
                bollard::container::RenameContainerOptions { name: managed.container_name().to_owned() }
            )
            .await?;

        tracing::trace!("Renamed container {}", managed.container_name());

        let mut filters = HashMap::new();
        filters.insert("id".to_owned(), vec![response.id.clone()]);
        let opts = EventsOptions {
            filters,
            ..Default::default()
        };
        
        let docker_id: Arc<str> = Arc::from(response.id);
        
        let subscription = self.docker.events(Some(opts));
        let listener = self
            .clone()
            .subscribe_container_events(subscription, managed.clone(), docker_id.clone());

        tracing::trace!("Subscribed to events for container '{}': {}", managed.container_name(), docker_id);

        let mut state = managed.state.lock().await;
        *state = Some(
            ManagedContainerState {
                docker_id,
                running: ManagedContainerRunning::Dead,
                listener,
            }
        );

        Ok(())
    }
    
    /// Spawn a new task that monitors a stream of Docker events for the given container
    fn subscribe_container_events(
        self: Arc<Self>,
        mut subscription: impl Stream<Item = Result<EventMessage, bollard::errors::Error>> + Send + Unpin + 'static,
        managed: Arc<ManagedContainer>,
        id: Arc<str>
    ) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn(async move {
            while let Some(event) = subscription.next().await {
                match event {
                    Ok(event) => self.clone().handle_container_event(managed.clone(), event).await,
                    Err(e) => {
                        tracing::error!("Failed to get Docker event for container {id}: {e}");
                    }
                }
            }
        })
    }

    async fn handle_container_event(self: Arc<Self>, managed: Arc<ManagedContainer>, event: EventMessage) {
        let Some(action) = event.action else { return };
        let Some(ref mut state) = *managed.state.lock().await else {
            tracing::warn!("Received event for deleted container {}", managed.container_name());
            return
        };

        tracing::info!("Container {} for {} got event '{}'", state.docker_id, managed.container_name(), action.as_str());

        match action.as_str() {
            "oom" => {
                tracing::error!("Container {} out of memory received - destroying container", state.docker_id);
                state.running = ManagedContainerRunning::Dead;
                self.clone().destroy(managed.clone());
            },
            "kill" | "die" => {
                state.running = ManagedContainerRunning::Dead;
                self.clone().destroy(managed.clone());
            },
            "paused" => {
                state.running = ManagedContainerRunning::Paused;
            },
            "unpause" => {
                state.running = ManagedContainerRunning::Running;
            }
            "start" => {
                state.running = ManagedContainerRunning::Running;
            },
            "stop" => {
                state.running = ManagedContainerRunning::Dead;
            }
            _ => {}
        }
    }
    
    /// Stop and remove the Docker container for the given managed container, and remove event
    /// listeners for the container
    async fn destroy(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let Some(state) = managed.state.lock().await.take() else { return Ok(()) };
        self.docker.stop_container(&state.docker_id, None).await?;
        self.docker.remove_container(
            &state.docker_id,
            Some(RemoveContainerOptions {
                force: false,
                ..Default::default()
            })
        ).await?;

        tracing::info!("Stopped and removed container {} for {}", state.docker_id, managed.container_name());

        state.listener.abort();

        tracing::trace!("Aborted event listener for {}", managed.container_name());

        Ok(())
    }
    
    /// Run all necessary tasks for the Docker container manager, cancel-safe with the given
    /// cancellation token
    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {
        tokio::select! {
            _ = cancel.cancelled() => {},
            _ = self.clone().run_internal() => {},
        };
        
        tracing::info!("Removing all Docker containers");
        self.stop_all().await;
    }

    async fn run_internal(self: Arc<Self>) {
        
    }

    /// Attempt to stop all running containers, e.g. for graceful server shutdown
    pub async fn stop_all(self: Arc<Self>) {
        let containers = self.containers.iter().map(|entry| entry.value().clone()).collect::<Vec<_>>();
        let tasks = containers
            .into_iter()
            .map(
                |container| tokio::task::spawn(self.clone().destroy(container))
            );

        for future in tasks {
            if let Err(e) = future.await {
                tracing::error!("Failed to spawn task to stop Docker container: {e}");
            }
        }
    }

    /// Get a handle to the connected Docker client
    pub fn client(&self) -> &Docker {
        &self.docker
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DockerInitError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Failed to load container configs from directory {dir}: {err}")]
    ContainersDirError {
        dir: PathBuf,
        err: std::io::Error
    },
    #[error("Duplicate configurations detected for docker container with name {0}")]
    DuplicateConfiguration(String),
}

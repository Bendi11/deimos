use std::{collections::HashMap, sync::Arc};

use bollard::{container::RemoveContainerOptions, secret::EventMessage, system::EventsOptions};
use container::{ManagedContainer, ManagedContainerError, ManagedContainerRunning, ManagedContainerState};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::Deimos;

pub mod container;
pub mod state;

pub use state::{DockerState, DockerInitError};

impl Deimos {
    /// Create a container from the configuration loaded for the given managed container
    pub async fn create_container(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let config = managed.docker_config();

        let response = self.docker.docker
            .create_container::<String, String>(
                None,
                config
            )
            .await?;

        tracing::trace!("Created container with ID {} for {}", response.id, managed.container_name());

        for warning in response.warnings {
            tracing::warn!("Warning when creating container {}: {}", managed.container_name(), warning);
        }

        self
            .docker
            .docker
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
        
        let subscription = self.docker.docker.events(Some(opts));
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
                    Ok(event) => self.clone().handle_container_event(id.clone(), managed.clone(), event).await,
                    Err(e) => {
                        tracing::error!("Failed to get Docker event for container {id}: {e}");
                    }
                }
            }
        })
    }
    
    /// Handle an event received from Docker, updating the state of the given container if required
    /// and sending status notifications to the gRPC server
    async fn handle_container_event(self: Arc<Self>, id: Arc<str>, managed: Arc<ManagedContainer>, event: EventMessage) {
        {
            let Some(action) = event.action else { return };
            let mut lock = managed.state.lock().await;
            let Some(ref mut state) = *lock else {
                tracing::warn!("Received event for deleted container {}", managed.container_name());
                return
            };


            tracing::info!("Container {} for {} got event '{}'", id, managed.container_name(), action.as_str());

            match action.as_str() {
                "oom" => {
                    tracing::error!("Container {} out of memory received - destroying container", state.docker_id);
                    state.running = ManagedContainerRunning::Dead;
                    drop(lock);
                    if let Err(e) = self.clone().destroy(managed.clone()).await {
                        tracing::error!("Failed to destroy container {}: {}", id, e);
                    }
                },
                "kill" | "die" => {
                    state.running = ManagedContainerRunning::Dead;
                    drop(lock);
                    if let Err(e) = self.clone().destroy(managed.clone()).await {
                        tracing::error!("Failed to destroy container {}: {}", id, e);
                    }
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
                _ => return
            }
        }
    }
    
    /// Stop and remove the Docker container for the given managed container, and remove event
    /// listeners for the container
    async fn destroy(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let Some(state) = managed.state.lock().await.take() else { return Ok(()) };
        self.docker.docker.stop_container(&state.docker_id, None).await?;
        self.docker.docker.remove_container(
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
    pub async fn docker_task(self: Arc<Self>, cancel: CancellationToken) {
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
        let containers = self.docker.containers.iter().map(|entry| entry.value().clone()).collect::<Vec<_>>();
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
}

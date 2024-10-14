use std::{collections::HashMap, sync::Arc, time::Duration};

use bollard::{container::{RemoveContainerOptions, StartContainerOptions, StopContainerOptions, WaitContainerOptions}, secret::EventMessage, system::EventsOptions};
use container::{ManagedContainer, ManagedContainerError, ManagedContainerRunning, ManagedContainerState};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::Deimos;

pub mod container;
pub mod state;

pub use state::{DockerState, DockerInitError};

impl Deimos {
    /// Create a container from the configuration loaded for the given managed container
    pub async fn create_container(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<Arc<str>, ManagedContainerError> {
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
                docker_id: docker_id.clone(),
                running: ManagedContainerRunning::Dead,
                listener,
            }
        );

        Ok(docker_id)
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
        let updated = self.clone().container_event_internal(id.clone(), managed.clone(), event).await;
        if updated {
            tracing::trace!("Notifying gRPC subscribers of status change for {}", id);
            let up_state = Self::container_api_run_status(&managed).await as i32;
            if let Err(e) = self.api.sender.send(deimosproto::ContainerStatusNotification { container_id: managed.container_name().to_string(), up_state }) {
                tracing::error!("Failed to send status notification to channel: {e}");
            }
        }
    }
    
    /// Modify the state of the managed container if necessary, and return `true` if we should send
    /// a notification of state change
    async fn container_event_internal(self: Arc<Self>, id: Arc<str>, managed: Arc<ManagedContainer>, event: EventMessage) -> bool {
        let Some(action) = event.action else { return false };
        let mut lock = managed.state.lock().await;
        let Some(ref mut state) = *lock else {
            tracing::warn!("Received event for deleted container {}", managed.container_name());
            return false
        };


        tracing::trace!("Container {} for {} got event '{}'", id, managed.container_name(), action.as_str());

        match action.as_str() {
            "oom" => {
                tracing::error!("Container {} out of memory received - destroying container", state.docker_id);
                state.running = ManagedContainerRunning::Dead;
                drop(lock);
                if let Err(e) = self.destroy(managed.clone()).await {
                    tracing::error!("Failed to destroy container {}: {}", id, e);
                }
                true
            },
            "destroy" => {
                *lock = None;
                true
            },
            "die" => {
                true
            },
            "kill" => {
                false
            },
            "paused" => {
                state.running = ManagedContainerRunning::Paused;
                true
            },
            "unpause" => {
                state.running = ManagedContainerRunning::Running;
                true
            }
            "start" => {
                state.running = ManagedContainerRunning::Running;
                true
            },
            "stop" => {
                state.running = ManagedContainerRunning::Dead;
                true
            }
            _ => false
        }
    }
    
    /// Stop and remove the Docker container for the given managed container, and remove event
    /// listeners for the container
    pub async fn destroy(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let mut lock = managed.state.lock().await;
        let Some(ref mut state) = *lock else { return Ok(()) };

        let handle = state.listener.abort_handle();
        let id = state.docker_id.clone();
        drop(lock);

        tracing::trace!("Waiting on container '{}' to stop", id);
        self.docker.docker.stop_container(&id, Some(StopContainerOptions { t: 60 * 3 })).await?;
        tracing::trace!("Container '{}' stopped", id);

        self.docker.docker.remove_container(
            &id,
            Some(RemoveContainerOptions {
                force: false,
                ..Default::default()
            })
        ).await?;

        tracing::info!("Stopped and removed container {} for {}", id, managed.container_name());

        tokio::task::yield_now().await;

        handle.abort();

        tracing::trace!("Aborted event listener for {}", managed.container_name());

        Ok(())
    }
    
    /// Create a Docker container for the given container if none exists, and start it
    pub async fn start(self: Arc<Self>, managed: Arc<ManagedContainer>) -> Result<(), ManagedContainerError> {
        let lock = managed.state.lock().await;
        let docker_id = match *lock {
            Some(ref state) => state.docker_id.clone(),
            None => {
                drop(lock);
                self.clone().create_container(managed.clone()).await?
            }
        };

        self
            .docker
            .docker
            .start_container(&docker_id, Option::<StartContainerOptions<String>>::None).await?;

        tracing::trace!("Starting container {} for '{}'", docker_id, managed.container_name());

        Ok(())
    }
    
    /// Run all necessary tasks for the Docker container manager, cancel-safe with the given
    /// cancellation token
    pub async fn docker_task(self: Arc<Self>, cancel: CancellationToken) {
        tokio::select! {
            _ = cancel.cancelled() => {},
        };
        
        tracing::info!("Removing all Docker containers");
        self.stop_all().await;
        tracing::trace!("Removed all Docker containers");
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

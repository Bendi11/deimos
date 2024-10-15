use std::{collections::HashMap, sync::Arc};

use bollard::{container::{RemoveContainerOptions, StartContainerOptions, StopContainerOptions}, secret::EventMessage, system::EventsOptions};
use container::{ManagedContainer, ManagedContainerError, ManagedContainerRunning, ManagedContainerState};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::Deimos;

pub mod container;
pub mod state;

pub use state::{DockerState, DockerInitError};

impl Deimos {
    /// Create a Docker container if it does not exist and start the given managed container
    pub async fn start(self: Arc<Self>, managed: Arc<ManagedContainer>, state: &mut Option<ManagedContainerState>) -> Result<Arc<str>, ManagedContainerError> {
        let docker_id = match state {
            Some(ref mut state) => match state.running {
                ManagedContainerRunning::Dead | ManagedContainerRunning::Paused => state.docker_id.clone(),
                ManagedContainerRunning::Running => {
                    return Ok(state.docker_id.clone())
                }
            },
            None => self.clone().create_container(managed.clone(), state).await?,
        };

        self
            .docker
            .docker
            .start_container(&docker_id, Option::<StartContainerOptions<String>>::None).await?;

        tracing::trace!("Starting container {} for '{}'", docker_id, managed.container_name());

        Ok(docker_id)
    }

    /// Create a container from the configuration loaded for the given managed container
    pub async fn create_container(self: Arc<Self>, managed: Arc<ManagedContainer>, state: &mut Option<ManagedContainerState>) -> Result<Arc<str>, ManagedContainerError> {
        if let Some(ref state) = state {
            return Ok(state.docker_id.clone())
        }

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
    
    /// Modify the state of the managed container if necessary, and return `true` if we should send
    /// a notification of state change
    async fn handle_container_event(self: Arc<Self>, id: Arc<str>, managed: Arc<ManagedContainer>, event: EventMessage) {
        let Some(action) = event.action else { return };
        let mut lock = managed.state.lock().await;
        let Some(ref mut state) = *lock else {
            tracing::trace!("Received event for deleted container {}", managed.container_name());
            return
        };


        tracing::trace!("Container {} for {} got event '{}'", id, managed.container_name(), action.as_str());

        let updated = match action.as_str() {
            "oom" => {
                tracing::error!("Container {} out of memory received - destroying container", state.docker_id);
                state.running = ManagedContainerRunning::Dead;
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
        };

        if updated {
            self.notify_status(managed.container_name(), &lock).await;
        }
    }
    
    /// Stop and remove the Docker container for the given managed container, and remove event
    /// listeners for the container
    pub async fn destroy(self: Arc<Self>, managed: Arc<ManagedContainer>, lock: &mut Option<ManagedContainerState>) -> Result<(), ManagedContainerError> {
        let Some(ref mut state) = lock else { return Ok(()) };

        let handle = state.listener.abort_handle();
        let id = state.docker_id.clone();

        tracing::trace!("Waiting on container '{}' to stop", id);
        self.docker.docker.stop_container(&id, Some(StopContainerOptions { t: 60 * 3 })).await?;

        self.docker.docker.remove_container(
            &id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            })
        ).await?;

        tracing::info!("Stopped and removed container {} for {}", id, managed.container_name());
         
        handle.abort();
        *lock = None;
        self.notify_status(managed.container_name(), lock).await;

        tracing::trace!("Aborted event listener for {}", managed.container_name());

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
                |container| {
                    let this = self.clone();
                    tokio::task::spawn(
                        async move {
                            let mut state = container.state.lock().await;
                            this.destroy(container.clone(), &mut state).await
                        }
                    )
                }
            );

        for future in tasks {
            if let Err(e) = future.await {
                tracing::error!("Failed to spawn task to stop Docker container: {e}");
            }
        }
    }
}

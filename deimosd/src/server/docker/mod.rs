use std::{collections::HashMap, sync::Arc};

use bollard::{
    container::{RemoveContainerOptions, StartContainerOptions, StopContainerOptions},
    secret::EventMessage,
    system::EventsOptions,
};
use container::{
    BollardError, DockerId, ManagedContainer, ManagedContainerError, ManagedContainerRunning,
    ManagedContainerShared, ManagedContainerTransaction,
};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use super::Deimos;

pub mod container;
pub mod state;

pub use state::{DockerInitError, DockerState};

impl Deimos {
    /// Create a Docker container if it does not exist and start the given managed container
    pub async fn start(
        self: Arc<Self>,
        tx: &mut ManagedContainerTransaction<'_>,
    ) -> Result<DockerId, ManagedContainerError> {
        let docker_id = match tx.state() {
            Some(ref state) => match state.running {
                ManagedContainerRunning::Dead | ManagedContainerRunning::Paused => {
                    state.docker_id.clone()
                }
                ManagedContainerRunning::Running => return Ok(state.docker_id.clone()),
            },
            None => self.clone().create_container(tx).await?,
        };

        self.docker
            .docker
            .start_container(&docker_id, Option::<StartContainerOptions<String>>::None)
            .await?;

        tracing::trace!(
            "Starting container {} for {}",
            docker_id,
            tx.container().deimos_id()
        );

        Ok(docker_id)
    }

    /// Create a container from the configuration loaded for the given managed container.
    /// Returns the ID of the container created on success
    pub async fn create_container(
        self: Arc<Self>,
        tx: &mut ManagedContainerTransaction<'_>,
    ) -> Result<DockerId, ManagedContainerError> {
        let managed = tx.container();
        if let Some(ref state) = tx.state() {
            return Ok(state.docker_id.clone());
        }

        let config = managed.docker_config();

        let response = self
            .docker
            .docker
            .create_container::<String, String>(None, config)
            .await?;

        tracing::trace!(
            "Created container with ID {} for {}",
            response.id,
            managed.deimos_id()
        );

        for warning in response.warnings {
            tracing::warn!(
                "Warning when creating container {}: {}",
                managed.deimos_id(),
                warning
            );
        }

        self.docker
            .docker
            .rename_container(
                &response.id,
                bollard::container::RenameContainerOptions {
                    name: managed.deimos_id().owned(),
                },
            )
            .await?;

        tracing::trace!("Renamed container {}", managed.deimos_id());

        let mut filters = HashMap::new();
        filters.insert("id".to_owned(), vec![response.id.clone()]);
        let opts = EventsOptions {
            filters,
            ..Default::default()
        };

        let docker_id = DockerId::from(response.id);

        let subscription = self.docker.docker.events(Some(opts));
        let listener = self.clone().subscribe_container_events(
            subscription,
            managed.clone(),
            docker_id.clone(),
        );

        let listener = Arc::new(listener);

        tracing::trace!(
            "Subscribed to events for container {}: {}",
            managed.deimos_id(),
            docker_id
        );

        tx.update(Some(ManagedContainerShared {
            docker_id: docker_id.clone(),
            running: ManagedContainerRunning::Dead,
            listener,
        }));

        Ok(docker_id)
    }

    /// Spawn a new task that monitors a stream of Docker events for the given container
    fn subscribe_container_events(
        self: Arc<Self>,
        mut subscription: impl Stream<Item = Result<EventMessage, bollard::errors::Error>>
            + Send
            + Unpin
            + 'static,
        managed: Arc<ManagedContainer>,
        id: DockerId,
    ) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn(async move {
            while let Some(event) = subscription.next().await {
                match event {
                    Ok(event) => {
                        self.clone()
                            .handle_container_event(id.clone(), managed.clone(), event)
                            .await
                    }
                    Err(e) => {
                        tracing::error!("Failed to get Docker event for container {id}: {e}");
                    }
                }
            }
        })
    }

    /// Modify the state of the managed container if necessary, and return `true` if we should send
    /// a notification of state change
    async fn handle_container_event(
        self: Arc<Self>,
        id: DockerId,
        managed: Arc<ManagedContainer>,
        event: EventMessage,
    ) {
        let Some(action) = event.action else { return };
        let mut tx = managed.transaction().await;

        tracing::trace!(
            "Container {} for {} got event '{}'",
            id,
            managed.deimos_id(),
            action.as_str()
        );

        let set_running = |running| {
            move |state: &mut Option<ManagedContainerShared>| {
                if let Some(ref mut state) = state {
                    state.running = running;
                }
            }
        };

        match action.as_str() {
            "oom" => {
                tracing::error!(
                    "Container {} out of memory received - destroying container",
                    id
                );
                if let Err(e) = self.destroy(&mut tx).await {
                    tracing::error!("Failed to destroy container after OOM received: {e}");
                }
            }
            "destroy" => {
                tx.update(None);
            }
            "die" => {}
            "kill" => {}
            "paused" => {
                tx.modify(set_running(ManagedContainerRunning::Paused));
            }
            "unpause" => {
                tx.modify(set_running(ManagedContainerRunning::Running));
            }
            "start" => {
                tx.modify(set_running(ManagedContainerRunning::Running));
            }
            "stop" => {
                tx.modify(set_running(ManagedContainerRunning::Running));
            }
            _ => (),
        };
    }

    /// Stop and remove the Docker container for the given managed container, and remove event
    /// listeners for the container
    pub async fn destroy(
        self: Arc<Self>,
        tx: &mut ManagedContainerTransaction<'_>,
    ) -> Result<(), ManagedContainerError> {
        let Some(state) = tx.state() else {
            return Ok(());
        };

        let handle = state.listener.abort_handle();
        let id = state.docker_id.clone();

        tracing::trace!("Waiting on container {} to stop", id);
        match self
            .docker
            .docker
            .stop_container(&id, Some(StopContainerOptions { t: 60 * 3 }))
            .await
        {
            Ok(_) => {
                tracing::trace!("Stopped container {}", id);
            }
            Err(BollardError::RequestTimeoutError) => {
                tracing::error!("");
            }
            Err(e) => return Err(e.into()),
        }

        self.docker
            .docker
            .remove_container(
                &id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        tracing::info!(
            "Stopped and removed container {} for {}",
            id,
            tx.container().deimos_id()
        );

        handle.abort();
        tx.update(None);

        tracing::trace!("Aborted event listener for {}", tx.container().deimos_id());

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
        let containers = self.docker.containers.values().cloned().collect::<Vec<_>>();
        let tasks = containers.into_iter().map(|container| {
            let this = self.clone();
            tokio::task::spawn(async move {
                let mut tx = container.transaction().await;
                if let Err(e) = this.destroy(&mut tx).await {
                    tracing::error!(
                        "Failed to destroy container {} while shutting down: {}",
                        tx.container().deimos_id(),
                        e
                    );
                }
            })
        });

        for future in tasks {
            if let Err(e) = future.await {
                tracing::error!("Failed to spawn task to stop Docker container: {e}");
            }
        }
    }
}

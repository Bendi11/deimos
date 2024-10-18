use std::{collections::HashMap, sync::Arc};

use bollard::{
    container::{RemoveContainerOptions, StartContainerOptions, StopContainerOptions},
    secret::EventMessage,
    system::EventsOptions,
};
use container::{
    BollardError, DockerId, ManagedContainer, ManagedContainerError, ManagedContainerDirective,
    ManagedContainerShared, ManagedContainerTransaction,
};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::server::upnp::UpnpLeaseData;

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
            Some(ref state) => match state.directive {
                ManagedContainerDirective::Stop | ManagedContainerDirective::Pause => {
                    state.docker_id.clone()
                }
                ManagedContainerDirective::Run => return Ok(state.docker_id.clone()),
            },
            None => self.clone().create_container(tx).await?,
        };

        self.docker
            .api
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

        let response = self
            .docker
            .api
            .create_container::<String, String>(None, managed.docker_config())
            .await?;

        for warning in response.warnings {
            tracing::warn!(
                "Warning when creating container {}: {}",
                managed.deimos_id(),
                warning
            );
        }

        let docker_id = DockerId::from(response.id);

        tracing::trace!(
            "Created container with ID {} for {}",
            docker_id,
            managed.deimos_id()
        );

        let listener = Arc::new(self.clone().subscribe_container_events(tx, docker_id.clone()));
        
        let forwarded_ports = managed
            .config
            .docker
            .port
            .iter()
            .filter(|port| port.upnp)
            .map(|port| UpnpLeaseData {
                    name: String::from(&*managed.config.name),
                    port: port.expose,
                    protocol: port.protocol.into(),
                }
            );

        let upnp_lease = self.upnp.request(forwarded_ports).await?;

        let state = ManagedContainerShared {
            docker_id,
            directive: ManagedContainerDirective::Stop,
            listener,
            upnp_lease,
        };

        tx.update(Some(state.clone()));

        if let Err(e) = self.docker
            .api
            .rename_container(
                &state.docker_id,
                bollard::container::RenameContainerOptions {
                    name: managed.deimos_id().owned(),
                },
            )
            .await {
            tracing::error!("Failed to rename docker container - destroying container");
            self.destroy(tx).await?;
            return Err(e.into())
        }

        tracing::trace!("Renamed container {} to {}", &state.docker_id, managed.deimos_id());

        Ok(state.docker_id)
    }

    /// Spawn a new task that monitors a stream of Docker events for the given container
    fn subscribe_container_events(
        self: Arc<Self>,
        tx: &ManagedContainerTransaction,
        id: DockerId,
    ) -> tokio::task::JoinHandle<()> {
        let container = tx.container().clone();

        let mut filters = HashMap::new();
        filters.insert("id".to_owned(), vec![id.owned()]);
        let opts = EventsOptions {
            filters,
            ..Default::default()
        };

        let mut subscription = self.docker.api.events(Some(opts));
        tracing::trace!(
            "Subscribed to events for container {}: {}",
            container.deimos_id(),
            id,
        );

        tokio::task::spawn(async move {
            while let Some(event) = subscription.next().await {
                match event {
                    Ok(event) => {
                        self.clone()
                            .handle_container_event(container.clone(), event)
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
        managed: Arc<ManagedContainer>,
        event: EventMessage,
    ) {
        // Only handle unexpected events - those that occur when there is no ongoing transaction
        let Some(mut tx) = managed.try_transaction() else { return };
        let Some(action) = event.action else { return };

        tracing::trace!(
            "Container {} got event '{}'",
            managed.deimos_id(),
            action.as_str()
        );

        let set_running = |running| {
            move |state: &mut Option<ManagedContainerShared>| {
                if let Some(ref mut state) = state {
                    state.directive = running;
                }
            }
        };

        match action.as_str() {
            "oom" => {
                tracing::error!("Container out of memory received - destroying container");
                if let Err(e) = self.destroy(&mut tx).await {
                    tracing::error!("Failed to destroy container after OOM received: {e}");
                }
            }
            "destroy" => {
                tx.update(None);
            }
            "die" => {
                tracing::warn!("Container {} unexpectedly died - destroying container", tx.container().deimos_id());
                tx.modify(set_running(ManagedContainerDirective::Stop));
                if let Err(e) = self.destroy(&mut tx).await {
                    tracing::error!("Failed to destroy container after unexpected die: {e}");
                }
            },
            "kill" => {},
            "paused" => {
                tx.modify(set_running(ManagedContainerDirective::Pause));
            },
            "unpause" => {
                tx.modify(set_running(ManagedContainerDirective::Run));
            }
            "start" => {
                tx.modify(set_running(ManagedContainerDirective::Run));
            }
            "stop" => {
                tx.modify(set_running(ManagedContainerDirective::Run));
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
        let Some(mut state) = tx.state() else {
            return Ok(());
        };

        let handle = state.listener.abort_handle();
        let id = state.docker_id.clone();

        tracing::trace!("Waiting on container {} to stop", id);
        match self
            .docker
            .api
            .stop_container(&id, Some(StopContainerOptions { t: 60 * 3 }))
            .await
        {
            Ok(_) => {
                tracing::trace!("Stopped container {}", id);
            }
            Err(e) => {
                tracing::error!("Error while stopping container {} for {}: {}", state.docker_id, tx.container().deimos_id(), e);
            }
        }

        state.directive = ManagedContainerDirective::Stop;
        tx.update(Some(state));

        self.docker
            .api
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

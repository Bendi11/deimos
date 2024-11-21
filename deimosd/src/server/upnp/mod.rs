use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use std::time::Duration;

use igd_next::aio::tokio::Tokio;
use igd_next::aio::Gateway;
use igd_next::PortMappingProtocol;
use tokio_util::sync::CancellationToken;

use super::Deimos;

/// State required to request port forwarding when the server is behind a NAT
#[derive(Clone)]
pub struct Upnp {
    /// Configuration parsed from the global deimos.toml
    conf: UpnpConfig,
    /// Transmitter sending new UPnP leases to the maintainer thread
    /// when they are accquired
    tx: tokio::sync::mpsc::Sender<UpnpMessage>,
    /// Local IP address, accquired from the local network interface
    local_ip: IpAddr,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct UpnpConfig {
    #[serde(default="UpnpConfig::default_renewal_seconds")]
    pub renewal_seconds: u32,
    #[serde(default)]
    pub remove_immediate: bool,
}

pub enum UpnpMessage {
    Add(UpnpLeaseData),
    Remove(u16)
}

pub type UpnpReceiver = tokio::sync::mpsc::Receiver<UpnpMessage>;
pub type UpnpSender = tokio::sync::mpsc::Sender<UpnpMessage>;

/// Data required to create a UPnP lease
#[derive(Debug, Clone)]
pub struct UpnpLeaseData {
    pub name: String,
    pub protocol: PortMappingProtocol,
    pub port: u16,
}

/// Tracking for the number of tasks that have requested the given port remain forwarded
#[derive(Debug)]
pub struct LeaseTrack {
    pub data: UpnpLeaseData,
    pub rc: usize,
}

/// Type representing a group of network ports mapped with UPNP to the device - maintains the lease
/// on a set interval and stops renewal when dropped
#[derive(Clone)]
pub struct UpnpLease {
    tx: UpnpSender,
    ports: Arc<[u16]>,
}

impl Deimos {
    /// Run a task to refresh all UPnP leases periodically
    pub async fn upnp_task(self: Arc<Self>, rx: UpnpReceiver, cancel: CancellationToken) {
        let task = self.upnp.task(rx);
        tokio::select! {
            _ = cancel.cancelled() => {},
            _ = task => {}
        }
    }
}

impl Upnp {
    /// Renew UPNP leases every fifteen minutes
    pub const RENEWAL_INTERVAL: Duration = Duration::from_secs(60 * 15);

    /// Retrieve the local IP address from the network adapter and create an empty map of forwarded
    /// ports
    pub async fn new(conf: UpnpConfig) -> Result<(Self, UpnpReceiver), UpnpInitError> {
        let local_ip = local_ip_address::local_ip()?;
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        Ok((
            Self {
                local_ip,
                tx,
                conf,
            },
            rx
        ))
    }
    
    /// Background task that requests UPnP leases of all ports from the gateway.
    /// This task must be running in order for UPnP leases to be actually accquired.
    pub async fn task(&self, mut rx: UpnpReceiver) {
        let gateway = match igd_next::aio::tokio::search_gateway(Default::default()).await {
            Ok(gateway) => gateway,
            Err(igd_next::SearchError::NoResponseWithinTimeout) => {
                tracing::warn!("No IGD enabled gateway located within timeout, port forwarding with UPnP will be disabled");
                return;
            }
            Err(e) => {
                tracing::error!("Failed to search IGD gateways: {e} - port forwarding with UPnP will be disabled");
                return;
            }
        };

        let mut renewal_interval = tokio::time::interval(Self::RENEWAL_INTERVAL);
        renewal_interval.tick().await;
        renewal_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut bound = HashMap::<u16, LeaseTrack>::new();

        loop {
            let msg = tokio::select! {
                _ = renewal_interval.tick() => {
                    for entry in bound.values() {
                        self.accquire(&gateway, &entry.data).await;
                    }

                    continue
                },
                Some(msg) = rx.recv() => msg,
            };

            match msg {
                UpnpMessage::Add(data) => match bound.get_mut(&data.port) {
                    Some(exist) => {
                        exist.rc += 1;
                    },
                    None => {
                        let port = data.port;
                        let track = LeaseTrack {
                            rc: 1,
                            data,
                        };
                        
                        self.accquire(&gateway, &track.data).await;
                        bound.insert(port, track);
                    }
                },
                UpnpMessage::Remove(port) => match bound.get_mut(&port) {
                    Some(entry) => {
                        entry.rc -= 1;
                        if entry.rc == 0 {
                            self.remove(&gateway, &entry.data).await;
                            bound.remove(&port);
                        }
                    },
                    None => {
                        tracing::warn!("Got UPnP remove port message for untracked port {}", port);
                    }
                }
            }
        }
    }

    async fn remove(&self, gateway: &Gateway<Tokio>, data: &UpnpLeaseData) {
        match gateway.remove_port(data.protocol, data.port).await {
            Ok(_) => {
                tracing::trace!("Removed UPnP lease {} for port {}", data.name, data.port);
            },
            Err(e) => {
                tracing::error!("Failed to remove UPnP lease for port {}: {}", data.port, e);
            }
        }
    }

    /// Request the given mapping from the IGD gateway
    async fn accquire(&self, gateway: &Gateway<Tokio>, lease: &UpnpLeaseData) {
        match gateway
            .add_port(
                lease.protocol,
                lease.port,
                SocketAddr::new(self.local_ip, lease.port),
                (Self::RENEWAL_INTERVAL.as_secs() + 60) as u32,
                &lease.name,
            )
            .await
        {
            Ok(_) => {
                tracing::trace!(
                    "Added UPNP lease for {} port {} named '{}'",
                    lease.protocol,
                    lease.port,
                    lease.name
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get UPNP lease for {} port {}: {}",
                    lease.protocol,
                    lease.port,
                    e
                );
            }
        }
    }

    /// Request the given block of UPnP leases, returning a structure that will maintain the ports
    /// mapped until it is dropped
    pub async fn request(
        &self,
        leases: Vec<UpnpLeaseData>,
    ) -> Result<UpnpLease, UpnpError> {
        let mut ports = Vec::with_capacity(leases.len());

        for data in leases {
            let port = data.port;
            ports.push(port);
            let _ = self.tx.send(UpnpMessage::Add(data)).await;
        }

        Ok(UpnpLease { tx: self.tx.clone(), ports: Arc::from(ports) })
    }
}

impl Default for UpnpConfig {
    fn default() -> Self {
        Self {
            renewal_seconds: Self::default_renewal_seconds(),
            remove_immediate: false,
        }
    }
}

impl UpnpConfig {
    pub const fn default_renewal_seconds() -> u32 {
        60 * 15
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpnpInitError {
    #[error("Failed to locate UPNP gateway: {0}")]
    Igd(#[from] igd_next::SearchError),
    #[error("Failed to retrieve local IP: {0}")]
    LocalIp(#[from] local_ip_address::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UpnpError {
    #[error("Port {0} already reserved with IGD gateway")]
    InUse(u16),
}

impl Drop for UpnpLease {
    fn drop(&mut self) {
        if let Some(ports) = Arc::get_mut(&mut self.ports) {
            let ports = Vec::from(ports);
            let tx = self.tx.clone();
            tokio::task::spawn(async move {
                for port in ports {
                    let _ = tx.send(UpnpMessage::Remove(port)).await;
                }
            });
        }
    }
}

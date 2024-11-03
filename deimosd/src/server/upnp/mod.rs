use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use std::time::Duration;

use dashmap::DashMap;
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
    tx: tokio::sync::mpsc::Sender<u16>,
    /// Local IP address, accquired from the local network interface
    local_ip: IpAddr,
    /// Map of all active UPnP leases
    leases: UpnpLeases,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct UpnpConfig {
    #[serde(default="UpnpConfig::default_renewal_seconds")]
    pub renewal_seconds: u32,
    #[serde(default)]
    pub remove_immediate: bool,
}

pub type UpnpReceiver = tokio::sync::mpsc::Receiver<u16>;

/// A reference to a mapping of active UPnP leases
#[derive(Clone, Default)]
struct UpnpLeases {
    map: Arc<DashMap<u16, UpnpLeaseData>>,
}

/// Data required to create a UPnP lease
#[derive(Debug, Clone)]
pub struct UpnpLeaseData {
    pub name: String,
    pub protocol: PortMappingProtocol,
    pub port: u16,
}

/// Type representing a group of network ports mapped with UPNP to the device - maintains the lease
/// on a set interval and stops renewal when dropped
#[derive(Clone)]
pub struct UpnpLease {
    leases: UpnpLeases,
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
        let leases = UpnpLeases::default();
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        Ok((
            Self {
                local_ip,
                leases,
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

        loop {
            tokio::select! {
                _ = renewal_interval.tick() => {
                    for entry in self.leases.map.iter() {
                        self.accquire(&gateway, entry.value()).await;
                    }
                },
                Some(new) = rx.recv() => {
                    match self.leases.map.get(&new) {
                        Some(ref lease) => self.accquire(&gateway, lease).await,
                        None => {
                            tracing::warn!("Notified of new UPnP lease that does not yet have a mapping");
                        }
                    }
                }
            };
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
        let lease = self.leases.add(leases).await?;
        for port in lease.ports.iter() {
            if let Err(e) = self.tx.send(*port).await {
                tracing::error!("Failed to send UPnP port update notification to UPnP maintainer: {}", e);
            }
        }

        Ok(lease)
    }
}

impl UpnpLeases {
    /// Request a new collection of ports to be forwarded
    pub async fn add(
        &self,
        ports: impl IntoIterator<Item = UpnpLeaseData>,
    ) -> Result<UpnpLease, UpnpError> {
        let lease_data = ports.into_iter().collect::<Vec<_>>();

        for data in lease_data.iter() {
            if self.map.contains_key(&data.port) {
                return Err(UpnpError::InUse(data.port));
            }
        }

        let ports = lease_data
            .iter()
            .map(|data| data.port)
            .collect::<Arc<[_]>>();
        for (port, data) in lease_data.into_iter().map(|data| (data.port, data)) {
            self.map.insert(port, data);
        }

        Ok(UpnpLease {
            leases: self.clone(),
            ports,
        })
    }

    /// Drop the given forwarded ports from the map.
    /// This function can be called from both async and non-async contexts - so `Drop`
    /// implementations can use it safely.
    pub fn drop(&self, ports: impl IntoIterator<Item = u16>) {
        for port in ports {
            self.map.remove(&port);
        }
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
        if let Some(ref ports) = Arc::get_mut(&mut self.ports) {
            self.leases.drop(ports.iter().copied())
        }
    }
}

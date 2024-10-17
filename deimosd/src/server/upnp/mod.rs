use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use std::time::Duration;

use igd_next::aio::tokio::Tokio;
use igd_next::aio::Gateway;
use igd_next::PortMappingProtocol;
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;

use super::Deimos;

/// State required to request ports with UPnP
pub struct Upnp {
    refresh: Notify,
    local_ip: IpAddr,
    leases: UpnpLeases,
}

#[derive(Clone, Default)]
struct UpnpLeases {
    map: Arc<Mutex<HashMap<u16, UpnpLeaseData>>>
}

/// Data required for UPNP lease
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
    ports: Arc<[u16]>
}


impl Deimos {
    /// Run a task to refresh all UPnP leases periodically
    pub async fn upnp_task(self: Arc<Self>, cancel: CancellationToken) {
        let task = self.upnp.task();
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
    pub async fn new() -> Result<Self, UpnpInitError> {
        let local_ip = local_ip_address::local_ip()?;
        let leases = UpnpLeases::default();
        let refresh = Notify::new();

        Ok(Self { local_ip, leases, refresh })
    }
    
    /// Task run to repeatedly renew all UPnP leases
    pub async fn task(&self) {
        let gateway = match igd_next::aio::tokio::search_gateway(Default::default()).await {
            Ok(gateway) => gateway,
            Err(igd_next::SearchError::NoResponseWithinTimeout) => {
                tracing::warn!("No IGD enabled gateway located within timeout, port forwarding with UPnP will be disabled");
                return
            },
            Err(e) => {
                tracing::error!("Failed to search IGD gateways: {e} - port forwarding with UPnP will be disabled");
                return
            },
        };

        let mut renewal_interval = tokio::time::interval(Self::RENEWAL_INTERVAL);

        loop {
            tokio::select! {
                _ = renewal_interval.tick() => {},
                _ = self.refresh.notified() => {
                    tracing::trace!("Refreshing UPnP leases in response to event");
                }
            };

            let lock = self.leases.map.lock().await;
            for (_, port) in lock.iter() {
                self.accquire(&gateway, port).await;
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
                tracing::trace!("Added UPNP lease for {} port {} named '{}'", lease.protocol, lease.port, lease.name);
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
    pub async fn request(&self, leases: impl IntoIterator<Item = UpnpLeaseData>) -> Result<UpnpLease, UpnpError> {
        let lease = self.leases.add(leases).await?;
        self.refresh.notify_one();

        Ok(lease)
    }
}

impl UpnpLeases {
    /// Request a new collection of ports to be forwarded
    pub async fn add(&self, ports: impl IntoIterator<Item = UpnpLeaseData>) -> Result<UpnpLease, UpnpError> {
        let lease_data = ports.into_iter().collect::<Vec<_>>();

        let mut map = self.map.lock().await;
        for data in lease_data.iter() {
            if map.contains_key(&data.port) {
                return Err(UpnpError::InUse(data.port))
            }
        }
        
        let ports = lease_data.iter().map(|data| data.port).collect::<Arc<[_]>>(); 
        map.extend(lease_data.into_iter().map(|data| (data.port, data)));
        
        Ok(
            UpnpLease {
                leases: self.clone(),
                ports,
            }
        )
    }
    
    /// Drop the given forwarded ports from the map.
    /// This function can be called from both async and non-async contexts - so `Drop`
    /// implementations can use it safely.
    pub fn drop(&self, ports: impl IntoIterator<Item = u16>) {
        let mut map = match tokio::runtime::Handle::try_current() {
            Ok(rt) => rt.block_on(self.map.lock()),
            Err(_) => self.map.blocking_lock()
        };

        for port in ports {
            map.remove(&port);
        }
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
        self.leases.drop(self.ports.iter().copied())
    }
}

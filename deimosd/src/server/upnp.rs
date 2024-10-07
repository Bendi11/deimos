use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use std::time::Duration;

use igd_next::aio::tokio::Tokio;
use igd_next::aio::Gateway;
use igd_next::PortMappingProtocol;

/// UPNP state
pub struct Upnp {
    gateway: Gateway<Tokio>,
    local_ip: IpAddr,
}

/// Type representing a group of network ports mapped with UPNP to the device - maintains the lease
/// on a set interval and stops renewal when dropped
pub struct UpnpLease(tokio::task::JoinHandle<()>);

impl Upnp {
    /// Renew UPNP leases every fifteen minutes
    pub const RENEWAL_INTERVAL: Duration = Duration::from_secs(60*15);
    
    /// Lookup the local gateway and retrieve the local IP address from the network adapter
    pub async fn new() -> Result<Self, UpnpInitError> {
        let gateway = igd_next::aio::tokio::search_gateway(Default::default()).await?;
        let local_ip = local_ip_address::local_ip()?;

        Ok(
            Self {
                gateway,
                local_ip,
            }
        )
    }

    pub async fn lease(&self, ports: impl Iterator<Item = (u16, PortMappingProtocol)>) -> Option<UpnpLease> {
        let gateway = self.gateway.clone();
        let ports = ports.collect::<Vec<_>>();
        let local_ip = self.local_ip;
        let mut interval = tokio::time::interval(Self::RENEWAL_INTERVAL);

        let task = async move {
            loop {
                interval.tick().await;
                for (port, protocol) in &ports {
                    match gateway.add_port(
                        *protocol,
                        *port,
                        SocketAddr::new(local_ip, *port),
                        (Self::RENEWAL_INTERVAL.as_secs() - 60) as u32,
                        "deimos"
                    ).await {
                        Ok(_) => {
                            tracing::trace!("Added UPNP lease for {} port {}", protocol, port);
                        },
                        Err(e) => {
                            tracing::warn!("Failed to get UPNP lease for {} port {}: {}", protocol, port, e);
                        }
                    }
                }
            };
        };

        tokio::task::spawn(task)
            .await
            .ok()
            .map(UpnpLease)
    }
}


#[derive(Debug, thiserror::Error)]
pub enum UpnpInitError {
    #[error("Failed to locate UPNP gateway: {0}")]
    Igd(#[from] igd_next::SearchError),
    #[error("Failed to retrieve local IP: {0}")]
    LocalIp(#[from] local_ip_address::Error),
}

impl Drop for UpnpLease {
    fn drop(&mut self) {
        self.0.abort()
    }
}

pub mod util;

mod proto {
    tonic::include_proto!("deimos");
}

#[cfg(feature = "server")]
pub use proto::deimos_service_server as server;
#[cfg(feature = "server")]
pub use proto::deimos_authorization_server as authserver;

#[cfg(feature = "channel")]
pub use proto::deimos_service_client as client;
#[cfg(feature = "channel")]
pub use proto::deimos_authorization_client as authclient;

pub use proto::*;

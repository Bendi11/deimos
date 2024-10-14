pub mod util;

mod proto  {
    tonic::include_proto!("deimos");
}

#[cfg(feature = "server")]
pub use proto::deimos_service_server::*;

#[cfg(feature = "channel")]
pub use proto::deimos_service_client::*;

pub use proto::*;

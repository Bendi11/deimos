pub mod util;

mod proto  {
    tonic::include_proto!("deimos");
}

#[cfg(feature = "server")]
pub use proto::deimos_server::*;

#[cfg(feature = "channel")]
pub use proto::deimos_client::*;

pub use proto::*;

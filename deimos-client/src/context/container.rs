use chrono::{DateTime, Utc};
use iced::widget::{image, Image};


/// Data received from a server about a single container, cached locally
#[derive(Debug)]
pub struct CachedContainerInfo {
    pub id: String,
    pub name: String,
    pub banner: Option<Image<image::Handle>>,
    pub icon: Option<Image<image::Handle>>,
    /// Date and time of the last time this container's data was fetched from the server
    pub last_update: DateTime<Utc>,
}

use std::time::Duration;

use http::Uri;
use iced::{alignment::Horizontal, widget::Text, Length, Padding, Pixels, Task};
use iced_aw::TypedInput;

use crate::context::ContextSettings;

use super::style::{Column, Container, Element};

#[derive(Debug, Clone)]
pub struct Settings {
    pub edited: ContextSettings,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    UpdatedServerUri(Uri),
    UpdatedRequestTimeout(u64),
}

impl Settings {
    /// Start a new settings editor given the current context's settings
    pub fn new(edited: ContextSettings) -> Self {
        Self { edited }
    }

    pub fn view(&self) -> Element<SettingsMessage> {
        let column = Column::new()
            .padding(Padding::default().top(30f32))
            .align_x(Horizontal::Center)
            .spacing(16f32)
            .max_width(Pixels(256f32))
            .push(
                Column::new().push(Text::new("Server URI")).push(
                    TypedInput::new("URL", &self.edited.server_uri)
                        .on_input(SettingsMessage::UpdatedServerUri)
                        .width(Length::Fill),
                ),
            )
            .push(
                Column::new().push(Text::new("gRPC Request Timeout")).push(
                    TypedInput::new("Timeout", &self.edited.request_timeout.as_secs())
                        .on_input(SettingsMessage::UpdatedRequestTimeout)
                        .width(Length::Fill),
                ),
            );

        Container::new(column)
            .center_x(Length::FillPortion(3))
            .into()
    }

    pub fn update(&mut self, msg: SettingsMessage) -> Task<SettingsMessage> {
        match msg {
            SettingsMessage::UpdatedServerUri(uri) => {
                self.edited.server_uri = uri;
            }
            SettingsMessage::UpdatedRequestTimeout(timeout) => {
                self.edited.request_timeout = Duration::from_secs(timeout);
            }
        }

        ().into()
    }
}

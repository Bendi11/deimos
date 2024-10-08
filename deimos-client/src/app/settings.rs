use std::time::Duration;

use http::Uri;
use iced::{alignment::Horizontal, widget::Text, Length, Padding, Pixels, Task};
use iced_aw::{Spinner, TypedInput};

use crate::context::ContextSettings;

use super::style::{Column, Container, Element};

#[derive(Debug, Clone)]
pub struct Settings(Option<SettingsInternal>);

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    Enter(ContextSettings),
    Internal(SettingsMessageInternal),
}

#[derive(Debug, Clone)]
struct SettingsInternal {
    pub edited: ContextSettings,
}

#[derive(Debug, Clone)]
pub enum SettingsMessageInternal {
    UpdatedServerUri(Uri),
    UpdatedRequestTimeout(u64),
}

impl Settings {
    /// Create a new settings widget with no populated context settings
    pub fn new() -> Self {
        Self(None)
    }
    
    /// Get the currently edited settings, to be used when saving context in Drop
    pub fn edited(&self) -> Option<ContextSettings> {
        self.0.clone().map(|s| s.edited)
    }
}

impl SettingsInternal {
    /// Start a new settings editor given the current context's settings
    pub fn new(edited: ContextSettings) -> Self {
        Self {
            edited,
        }
    }

    pub fn view(&self) -> Element<SettingsMessageInternal> {
        let column = Column::new()
            .padding(Padding::default().top(30f32))
            .align_x(Horizontal::Center)
            .spacing(16f32)
            .max_width(Pixels(256f32))
            .push(
                Column::new().push(Text::new("Server URI")).push(
                    TypedInput::new("URL", &self.edited.server_uri)
                        .on_input(SettingsMessageInternal::UpdatedServerUri)
                        .width(Length::Fill),
                ),
            )
            .push(
                Column::new().push(Text::new("gRPC Request Timeout")).push(
                    TypedInput::new("Timeout", &self.edited.request_timeout.as_secs())
                        .on_input(SettingsMessageInternal::UpdatedRequestTimeout)
                        .width(Length::Fill),
                ),
            );

        Container::new(column)
            .center_x(Length::FillPortion(3))
            .into()
    }

    pub fn update(&mut self, msg: SettingsMessageInternal) -> Task<SettingsMessageInternal> {
        match msg {
            SettingsMessageInternal::UpdatedServerUri(uri) => {
                self.edited.server_uri = uri;
            }
            SettingsMessageInternal::UpdatedRequestTimeout(timeout) => {
                self.edited.request_timeout = Duration::from_secs(timeout);
            }
        }

        ().into()
    }
}

impl Settings {
    pub fn view(&self) -> Element<SettingsMessage> {
        match self.0 {
            Some(ref s) => s.view().map(SettingsMessage::Internal),
            None => Spinner::new().into()
        }
    }

    pub fn update(&mut self, msg: SettingsMessage) -> Task<SettingsMessage> {
        match msg {
            SettingsMessage::Enter(s) => {
                self.0 = Some(SettingsInternal::new(s));
                Task::none()
            },
            SettingsMessage::Internal(internal) => match self.0 {
                Some(ref mut s) => s.update(internal).map(SettingsMessage::Internal),
                None => {
                    tracing::warn!("Settings got internal message before context could provide current settings");
                    Task::none()
                }
            }
        }
    }
}

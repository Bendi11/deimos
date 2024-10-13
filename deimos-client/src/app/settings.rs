use std::time::Duration;

use http::Uri;
use iced::{alignment::Horizontal, widget::Text, Length, Padding, Pixels, Task};
use iced_aw::TypedInput;

use crate::context::Context;

use super::style::{Column, Container, Element};


#[derive(Debug, Clone)]
pub struct Settings;

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    ServerUri(Uri),
    RequestTimeout(u64),
    ConnectTimeout(u64),
}


impl Settings {
    pub fn new() -> Self {
        Self
    }

    pub fn view(&self, ctx: &Context) -> Element<SettingsMessage> {
        let column = Column::new()
            .padding(Padding::default().top(30f32))
            .align_x(Horizontal::Center)
            .spacing(16f32)
            .max_width(Pixels(256f32))
            .push(
                Column::new().push(Text::new("Server URI")).push(
                    TypedInput::new("URL", &ctx.state.settings.server_uri)
                        .on_input(SettingsMessage::ServerUri)
                        .width(Length::Fill),
                ),
            )
            .push(
                Column::new().push(Text::new("gRPC Request Timeout")).push(
                    TypedInput::new("Timeout", &ctx.state.settings.request_timeout.as_secs())
                        .on_input(SettingsMessage::RequestTimeout)
                        .width(Length::Fill),
                ),
            )
            .push(
                Column::new().push(Text::new("gRPC Connect Timeout")).push(
                    TypedInput::new("Timeout", &ctx.state.settings.connect_timeout.as_secs())
                        .on_input(SettingsMessage::ConnectTimeout)
                        .width(Length::Fill)
                )
            );

        Container::new(column)
            .center_x(Length::FillPortion(3))
            .into()
    }

    pub fn update(&mut self, ctx: &mut Context, msg: SettingsMessage) -> Task<SettingsMessage> {
        match msg {
            SettingsMessage::ServerUri(uri) => {
                ctx.state.settings.server_uri = uri;
            }
            SettingsMessage::RequestTimeout(timeout) => {
                ctx.state.settings.request_timeout = Duration::from_secs(timeout);
            },
            SettingsMessage::ConnectTimeout(timeout) => {
                ctx.state.settings.connect_timeout = Duration::from_secs(timeout);
            }
        }

        iced::Task::none()
    }
}

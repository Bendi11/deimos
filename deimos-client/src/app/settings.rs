use std::{str::FromStr, sync::Arc};

use http::Uri;
use iced::{alignment::Horizontal, widget::{svg, Space, Text}, Length, Pixels, Task};

use crate::context::{Context, ContextSettings};

use super::{style::{Button, Column, Container, Element, Svg, TextInput}, DeimosView};


#[derive(Debug)]
pub struct Settings {
    ctx: Arc<Context>,
    edited_uri: String,
    icon: svg::Handle,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    UpdatedServerUri(String),
}

impl Settings {
    const ICON_SVG: &[u8] = include_bytes!("../../assets/settings.svg");

    pub fn new(ctx: Arc<Context>) -> Self {
        Self {
            edited_uri: ctx.settings().server_uri.to_string(),
            ctx,
            icon: svg::Handle::from_memory(Self::ICON_SVG)
        }
    }

    pub fn icon(&self) -> Element<DeimosView> {
        Button::new(
            Svg::new(self.icon.clone())
                .class((super::style::orbit::MERCURY[1], super::style::orbit::SOL[0]))
                .width(Length::Shrink)
        )
            .on_press(DeimosView::Settings)
            .into()
    }

    pub fn view(&self) -> Element<SettingsMessage> {
        let column = Column::new()
            .align_x(Horizontal::Center)
            .push(Space::new(Length::Fill, Length::Fixed(30f32)))
            .push(Text::new("Server URI"))
            .push(
                TextInput::new("", &self.edited_uri)
                    .on_input(|txt| SettingsMessage::UpdatedServerUri(txt))
            )
            .max_width(Pixels(256f32));
        
        Container::new(column)
            .center_x(Length::FillPortion(3))
            .into()
    }

    pub fn update(&mut self, msg: SettingsMessage) -> Task<SettingsMessage> {
        match msg {
            SettingsMessage::UpdatedServerUri(uri) => {
                self.edited_uri = uri;
                ().into()
            }
        }
    }
}

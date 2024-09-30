use std::sync::Arc;

use iced::{widget::{svg, Text}, Task};

use crate::context::Context;

use super::{style::{Button, Element, Svg, Theme}, DeimosView};


#[derive(Debug)]
pub struct Settings {
    ctx: Arc<Context>,
    icon: svg::Handle,
}

#[derive(Debug)]
pub enum SettingsMessage {

}

impl Settings {
    const ICON_SVG: &[u8] = include_bytes!("../../assets/settings.svg");

    pub fn new(ctx: Arc<Context>) -> Self {
        Self {
            ctx,
            icon: svg::Handle::from_memory(Self::ICON_SVG)
        }
    }

    pub fn icon(&self) -> Element<DeimosView> {
        Button::new(
            Svg::new(self.icon.clone())
                .class((super::style::orbit::MERCURY[1], super::style::orbit::SOL[0]))
        )
            .on_press(DeimosView::Settings)
            .into()
    }

    pub fn view(&self) -> Element<SettingsMessage> {
        Text::new("Test")
            .into()
    }

    pub fn update(&mut self, msg: SettingsMessage) -> Task<SettingsMessage> {
        Task::none() 
    }
}

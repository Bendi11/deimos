
use iced::Length;

use super::{style::{Column, Element, Text}, DeimosApplication, DeimosMessage};



impl DeimosApplication {
    pub fn sidebar(&self) -> Element<DeimosMessage> {
        Column::new()
            .width(Length::FillPortion(1))
            .push(
                Text::new("Deimos")
                    .width(Length::Fill)
            )
            .into()
    }
}

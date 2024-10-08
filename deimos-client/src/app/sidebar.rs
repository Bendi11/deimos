use std::sync::Arc;

use iced::{alignment::Horizontal, border::Radius, widget::svg, Background, Length, Padding, Shadow, Task, Vector};

use crate::context::Context;

use super::style::{container::ContainerClass, orbit, Column, Container, Element, Row, Svg, Text};

#[derive(Debug)]
pub struct Sidebar {
    ctx: Arc<Context>,
    icon: svg::Handle,
}

#[derive(Debug, Clone)]
pub enum SidebarMessage {

}

impl Sidebar {
    pub fn new(ctx: Arc<Context>) -> Self {
        Self {
            ctx,
            icon: svg::Handle::from_memory(include_bytes!("../../assets/mars-deimos.svg")),
        }
    }

    pub fn view(&self) -> Element<SidebarMessage> {
        let header = Row::new()
            .push(
                Svg::new(self.icon.clone())
                    .class(orbit::MARS[1])
                    .height(64f32)
                    .width(Length::FillPortion(1)),
            )
            .push(
                Column::new()
                    .push(
                        Text::new("Deimos")
                            .size(30f32)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .center(),
                    )
                    .align_x(Horizontal::Center)
                    .width(Length::FillPortion(1)),
            )
            .padding(Padding::default().top(16f32).left(16f32).right(16f32))
            .height(128);


        Container::new(Column::new().push(header))
            .class(ContainerClass {
                radius: Radius {
                    top_left: 0f32,
                    top_right: 5f32,
                    bottom_right: 5f32,
                    bottom_left: 0f32,
                },
                background: Some(Background::Color(orbit::NIGHT[1])),
                shadow: Some(Shadow {
                    color: orbit::NIGHT[3],
                    offset: Vector::new(1f32, 0f32),
                    blur_radius: 16f32,
                }),
            })
            .width(Length::Fixed(256f32))
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, msg: SidebarMessage) -> Task<SidebarMessage> {
        match msg {

        }
    }
}

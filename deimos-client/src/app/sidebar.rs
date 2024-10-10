use std::sync::Arc;

use iced::{alignment::{Horizontal, Vertical}, border::Radius, gradient::Linear, widget::svg, Background, ContentFit, Degrees, Gradient, Length, Padding, Radians, Shadow, Task, Vector};

use crate::context::{container::CachedContainer, Context};

use super::style::{button::ButtonClass, container::ContainerClass, orbit, Button, Column, Container, Element, Row, Svg, Text};

#[derive(Debug)]
pub struct Sidebar {
    icon: svg::Handle,
    reload: svg::Handle,
}

#[derive(Debug, Clone)]
pub enum SidebarMessage {
    Refresh,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            icon: svg::Handle::from_memory(include_bytes!("../../assets/mars-deimos.svg")),
            reload: svg::Handle::from_memory(include_bytes!("../../assets/reload.svg")),
        }
    }

    pub fn view<'a>(&self, ctx: &'a Context) -> Element<'a, SidebarMessage> {
        let header = Row::new()
            .push(
                Svg::new(self.icon.clone())
                    .class(orbit::MARS[1])
                    .width(Length::FillPortion(1)),
            )
            .push(
                Column::new()
                    .spacing(8)
                    .align_x(Horizontal::Center)
                    .width(Length::FillPortion(1))
                    .push(
                        Text::new("Deimos")
                            .size(30f32)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .center(),
                    )
                    .push(
                        Container::new(
                            Button::new(
                                Svg::new(self.reload.clone())
                                    .class((orbit::MERCURY[3], orbit::SOL[0]))
                                    .height(32f32)
                                    .width(32f32)
                            )
                            .on_press(SidebarMessage::Refresh)
                        )
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(Horizontal::Right)
                            .align_y(Vertical::Bottom)
                    )

            )
            .padding(
                Padding::default()
                    .top(16f32)
                    .left(0f32)
                    .right(16f32)
            )
            .height(100);

        let mut containers = Column::new()
            .height(Length::FillPortion(8))
            .spacing(16)
            .padding(
                Padding::default()
                    .left(10f32)
                    .right(10f32)
            );

        for (_, container) in ctx.containers.iter() {
            containers = containers.push(Self::container_button(container));
        }


        let top = Column::new()
            .spacing(32)
            .push(header)
            .push(containers);

        Container::new(top)
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
            other => panic!("Message {:?} sent to sidebar", other),
        }
    }

    fn container_button(container: &CachedContainer) -> Element<SidebarMessage> {
        let class = ContainerClass {
            background: Some(
                Background::Color(orbit::NIGHT[0])
            ),
            radius: Radius::new(4f32),
            shadow: Some(
                Shadow {
                    color: orbit::NIGHT[2],
                    offset: Vector::ZERO,
                    blur_radius: 4f32
                }
            ),
        };

        Container::new(
            Button::new(
                Text::new(&container.data.name)
            )
        )
        .align_right(Length::Fill)
        .center_y(Length::Fill)
        .class(class)
        .height(64f32)
        .width(Length::Fill)
        .into()
    }
}

use std::sync::Arc;

use iced::{alignment::Horizontal, border::Radius, gradient::Linear, widget::svg, Background, ContentFit, Degrees, Gradient, Length, Padding, Radians, Shadow, Task, Vector};

use crate::context::Context;

use super::style::{button::ButtonClass, container::ContainerClass, orbit, Button, Column, Container, Element, Row, Svg, Text};

#[derive(Debug)]
pub struct Sidebar {
    icon: svg::Handle,
    reload: svg::Handle,
    container_entries: Vec<SidebarEntry>,
}

#[derive(Debug, Clone)]
pub struct SidebarEntry {
    pub name: Arc<str>,
    pub running: bool,
}

#[derive(Debug, Clone)]
pub enum SidebarMessage {
    Refresh,
    ContainerEntries(Vec<SidebarEntry>),
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            icon: svg::Handle::from_memory(include_bytes!("../../assets/mars-deimos.svg")),
            reload: svg::Handle::from_memory(include_bytes!("../../assets/reload.svg")),
            container_entries: Vec::new(),
        }
    }

    pub fn view(&self) -> Element<SidebarMessage> {
        let header = Row::new()
            .push(
                Svg::new(self.icon.clone())
                    .class(orbit::MARS[1])
                    .width(Length::FillPortion(1)),
            )
            .push(
                Column::new()
                    .spacing(16)
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
                                    .content_fit(ContentFit::Contain)
                                    .class((orbit::MERCURY[3], orbit::SOL[0]))
                            )
                            .on_press(SidebarMessage::Refresh)
                            .padding(Padding::default().left(85))
                        ).align_right(Length::Fill)
                    )

            )
            .padding(
                Padding::default()
                    .top(16f32)
                    .left(0f32)
                    .right(16f32)
            )
            .height(100);

        let mut col = Column::new()
            .padding(Padding::default().left(10).right(10))
            .spacing(16)
            .push(header);

        for entry in &self.container_entries {
            col = col.push(entry.view());
        }

        Container::new(col)
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
            SidebarMessage::ContainerEntries(entries) => {
                self.container_entries = entries;
                Task::none()
            },
            other => panic!("Message {:?} sent to sidebar", other),
        }
    }
}

impl SidebarEntry {
    pub fn view(&self) -> Element<SidebarMessage> {
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
                Text::new(&*self.name)
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

use std::sync::Arc;

use iced::{
    alignment::{Horizontal, Vertical},
    border::Radius,
    widget::svg,
    Background, Length, Padding, Shadow, Task, Vector,
};

use crate::context::{
    container::{CachedContainer, CachedContainerUpState, CachedContainerUpStateFull},
    ContainerRef, Context,
};

use super::{
    style::{container::ContainerClass, orbit, Button, Column, Container, Element, Row, Svg, Text},
    DeimosMessage,
};

#[derive(Debug)]
pub struct Sidebar {
    icon: svg::Handle,
    reload: svg::Handle,
    start: svg::Handle,
    stop: svg::Handle,
}

#[derive(Debug, Clone)]
pub enum SidebarMessage {
    Refresh,
    SelectContainer(ContainerRef),
    UpdateContainer(ContainerRef, CachedContainerUpState),
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            icon: svg::Handle::from_memory(include_bytes!("../../assets/mars-deimos.svg")),
            reload: svg::Handle::from_memory(include_bytes!("../../assets/reload.svg")),
            start: svg::Handle::from_memory(include_bytes!("../../assets/start.svg")),
            stop: svg::Handle::from_memory(include_bytes!("../../assets/stop.svg")),
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
                                    .width(32f32),
                            )
                            .on_press(SidebarMessage::Refresh),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Horizontal::Right)
                        .align_y(Vertical::Bottom),
                    ),
            )
            .padding(Padding::default().top(16f32).left(0f32).right(16f32))
            .height(100);

        let mut containers = Column::new()
            .height(Length::FillPortion(8))
            .spacing(16)
            .padding(Padding::default().left(10f32).right(10f32));

        for (r, container) in ctx.containers.iter() {
            containers = containers.push(self.container_button(r, container));
        }

        let top = Column::new().spacing(32).push(header).push(containers);

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

    pub fn update(&mut self, ctx: &mut Context, msg: SidebarMessage) -> Task<SidebarMessage> {
        match msg {
            other => panic!("Message {:?} sent to sidebar", other),
        }
    }

    fn container_button<'a>(
        &self,
        r: ContainerRef,
        container: &'a CachedContainer,
    ) -> Element<'a, SidebarMessage> {
        let (msg, svg) = match container.data.up {
            CachedContainerUpStateFull::Known(ref state) => match state {
                CachedContainerUpState::Dead => (
                    Some(SidebarMessage::UpdateContainer(
                        r,
                        CachedContainerUpState::Running,
                    )),
                    Svg::new(self.start.clone()).class(orbit::MERCURY[2]),
                ),
                CachedContainerUpState::Paused => (
                    Some(SidebarMessage::UpdateContainer(
                        r,
                        CachedContainerUpState::Running,
                    )),
                    Svg::new(self.start.clone()).class(orbit::MERCURY[2]),
                ),
                CachedContainerUpState::Running => (
                    Some(SidebarMessage::UpdateContainer(
                        r,
                        CachedContainerUpState::Dead,
                    )),
                    Svg::new(self.stop.clone()).class(orbit::MARS[1]),
                ),
            },
            CachedContainerUpStateFull::UpdateRequested { .. } => {
                (None, Svg::new(self.reload.clone()).class(orbit::EARTH[2]))
            }
        };

        let row = Row::new()
            .push(
                Button::new(Text::new(&container.data.name))
                    .on_press(SidebarMessage::SelectContainer(r))
                    .height(Length::Fill)
                    .width(Length::FillPortion(3)),
            )
            .push(
                Container::new(
                    Button::new(svg.width(Length::Fill).height(Length::Fill))
                        .on_press_maybe(msg)
                        .height(Length::Fill)
                        .width(Length::FillPortion(1)),
                )
                .class(ContainerClass {
                    background: None,
                    radius: Radius::new(0f32),
                    shadow: Some(Shadow {
                        color: orbit::NIGHT[2],
                        offset: Vector::new(-0.5f32, 0f32),
                        blur_radius: 3f32,
                    }),
                }),
            );

        Container::new(row)
            .height(60f32)
            .width(Length::Fill)
            .class(ContainerClass {
                background: Some(Background::Color(orbit::NIGHT[0])),
                radius: Radius::new(4f32),
                shadow: Some(Shadow {
                    color: orbit::NIGHT[2],
                    offset: Vector::ZERO,
                    blur_radius: 5f32,
                }),
            })
            .into()
    }
}

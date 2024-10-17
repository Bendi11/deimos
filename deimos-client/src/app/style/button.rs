use iced::{border::Radius, widget::button, Background, Border, Shadow, Vector};

use super::{container::ContainerClass, Theme};

#[derive(Default, Clone, Copy)]
pub struct ButtonClass {
    pub normal: ContainerClass,
    pub hovered: ContainerClass,
    pub pressed: ContainerClass,
}

impl button::Catalog for Theme {
    type Class<'a> = ButtonClass;

    fn default<'a>() -> Self::Class<'a> {
        ButtonClass::from(ContainerClass::default())
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class.style(status)
    }
}

impl ButtonClass {
    fn style(&self, status: button::Status) -> button::Style {
        match status {
            button::Status::Hovered => Self::container_to_button(&self.hovered),
            button::Status::Pressed => Self::container_to_button(&self.pressed),
            _ => Self::container_to_button(&self.normal),
        }
    }

    fn container_to_button(class: &ContainerClass) -> button::Style {
        button::Style {
            border: Border {
                radius: class.radius,
                width: 0f32,
                color: iced::Color::BLACK,
            },
            background: class.background,
            shadow: class.shadow.unwrap_or_default(),
            ..Default::default()
        }
    }

    fn scale_color(color: iced::Color, modify: iced::Color) -> iced::Color {
        iced::Color::from_rgba(
            color.r - color.r * modify.r,
            color.g - color.g * modify.g,
            color.b - color.b * modify.b,
            color.a - color.a * modify.a,
        )
    }

    fn modify_background(bg: iced::Background, modify: iced::Color) -> iced::Background {
        match bg {
            Background::Color(c) => Background::Color(Self::scale_color(c, modify)),
            Background::Gradient(gradient) => match gradient {
                iced::Gradient::Linear(linear) => {
                    Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear {
                        angle: linear.angle,
                        stops: linear.stops.map(|s| {
                            s.map(|stop| iced::gradient::ColorStop {
                                offset: stop.offset,
                                color: Self::scale_color(stop.color, modify),
                            })
                        }),
                    }))
                }
            },
        }
    }
}

impl From<ContainerClass> for ButtonClass {
    fn from(value: ContainerClass) -> Self {
        Self {
            normal: value,
            hovered: ContainerClass {
                radius: value.radius,
                background: Some(
                    value
                        .background
                        .map(|c| Self::modify_background(c, iced::Color::from_rgb(0.8, 0.8, 0.8)))
                        .unwrap_or(Background::Color(iced::Color::from_rgba(0., 0., 0., 0.2))),
                ),
                shadow: value.shadow,
            },
            pressed: ContainerClass {
                radius: value.radius,
                background: Some(
                    value
                        .background
                        .map(|c| {
                            Self::modify_background(c, iced::Color::from_rgb(0.75, 0.75, 0.75))
                        })
                        .unwrap_or(Background::Color(iced::Color::from_rgba(0., 0., 0., 0.25))),
                ),
                shadow: value.shadow,
            },
        }
    }
}

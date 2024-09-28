use iced::{border::Radius, widget::container, Background, Border, Shadow, Vector};

use super::Theme;

pub enum ContainerClass {
    RoundedBorder,
}

impl container::Catalog for Theme {
    type Class<'a> = ContainerClass;

    fn default<'a>() -> Self::Class<'a> {
        ContainerClass::RoundedBorder
    }

    fn style(&self, class: &Self::Class<'_>) -> container::Style {
        match class {
            ContainerClass::RoundedBorder => container::Style {
                text_color: None,
                background: Some(Background::Color(self.bg_light)),
                border: Border {
                    color: self.rule,
                    width: 2f32,
                    radius: Radius::from(1f32),
                },
                shadow: Shadow {
                    color: iced::Color::BLACK,
                    offset: Vector::new(0f32, 2f32),
                    blur_radius: 2f32,
                }
            }
        }
    }
}

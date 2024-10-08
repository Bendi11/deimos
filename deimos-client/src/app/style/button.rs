use iced::{border::Radius, widget::button, Background, Border, Shadow, Vector};

use super::{container::ContainerClass, Theme};

#[derive(Default, Clone, Copy)]
pub struct ButtonClass {
    pub normal: ContainerClass,
    pub hovered: ContainerClass,
}

impl button::Catalog for Theme {
    type Class<'a> = ButtonClass;

    fn default<'a>() -> Self::Class<'a> {
        ButtonClass::default()
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class.style(status)
    }
}

impl ButtonClass {
    fn style(&self, status: button::Status) -> button::Style {
        match status {
            button::Status::Hovered => Self::container_to_button(&self.hovered),
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
}

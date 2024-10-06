use iced::{border::Radius, widget::container, Background, Border, Color, Shadow, Vector};

use super::Theme;

pub struct ContainerClass {
    pub radius: Radius,
    pub background: Option<Background>,
    pub shadow: Option<Shadow>,
}

impl container::Catalog for Theme {
    type Class<'a> = ContainerClass;

    fn default<'a>() -> Self::Class<'a> {
        ContainerClass {
            radius: Default::default(),
            background: None,
            shadow: None,
        }
    }

    fn style(&self, class: &Self::Class<'_>) -> container::Style {
        container::Style {
            text_color: None,
            border: Border {
                radius: class.radius,
                ..Default::default()
            },
            background: class.background,
            shadow: class.shadow.unwrap_or_default(),
        }
    }
}

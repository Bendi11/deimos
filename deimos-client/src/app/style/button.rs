use iced::{border::Radius, widget::button, Background, Shadow, Vector};

use super::Theme;

#[derive(Clone, Copy, Debug)]
pub enum ButtonClass {
    Rounded,
    SquareBox,
}

impl button::Catalog for Theme {
    type Class<'a> = ButtonClass;

    fn default<'a>() -> Self::Class<'a> {
        ButtonClass::Rounded
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class.style(status)
    }
}

impl ButtonClass {
    fn style(&self, status: button::Status) -> button::Style {
        button::Style {
            ..Default::default()
        }
    }
}

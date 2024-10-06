use iced::{border::Radius, widget::text_input, Background};

use super::Theme;

pub enum TextInputClass {
    SingleLine,
}

impl text_input::Catalog for Theme {
    type Class<'a> = TextInputClass;

    fn default<'a>() -> Self::Class<'a> {
        Self::Class::SingleLine
    }

    fn style(&self, _: &Self::Class<'_>, status: text_input::Status) -> text_input::Style {
        text_input::Style {
            background: Background::Color(self.bg_bright),
            border: iced::Border {
                color: match status {
                    text_input::Status::Focused => self.active,
                    _ => self.text_dim
                },
                width: 2f32,
                radius: Radius::from(5f32)
            },
            icon: self.text_bright,
            placeholder: self.text_dim,
            value: self.text_normal,
            selection: self.warn
        }    
    }
}

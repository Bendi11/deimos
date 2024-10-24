use iced::{border::Radius, widget::text_input, Background, Border};

use super::{orbit, Theme};

pub struct TextInputClass {
    pub border: Border,
}

impl text_input::Catalog for Theme {
    type Class<'a> = TextInputClass;

    fn default<'a>() -> Self::Class<'a> {
        TextInputClass::default()
    }

    fn style(&self, class: &Self::Class<'_>, _: text_input::Status) -> text_input::Style {
        text_input::Style {
            background: Background::Color(self.bg_light),
            border: class.border,
            icon: self.text_bright,
            placeholder: self.text_dim,
            value: self.text_normal,
            selection: self.active,
        }
    }
}

impl Default for TextInputClass {
    fn default() -> Self {
        Self {
            border: Border {
                width: 0f32,
                radius: Radius::from(3f32),
                color: iced::Color::BLACK,
            }
        }
    }
}

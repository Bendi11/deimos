use iced::{border::Radius, widget::button, Background, Shadow, Vector};

use super::Theme;

impl button::StyleSheet for Theme {
    type Style = ();

    fn active(&self, _: &Self::Style) -> button::Appearance {
        button::Appearance {
            shadow_offset: Vector::ZERO,
            background: Some(Background::Color(self.bg_bright)),
            text_color: self.text_bright,
            border: iced::Border {
                color: self.rule,
                width: 1f32,
                radius: Radius::from(1f32),
            },
            shadow: Shadow {
                color: iced::Color::BLACK,
                offset: Vector::ZERO,
                blur_radius: 0f32,
            }
        }
    }
}

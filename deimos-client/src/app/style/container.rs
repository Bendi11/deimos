use iced::{border::Radius, widget::container, Background, Border, Shadow, Vector};

use super::Theme;

impl container::StyleSheet for Theme {
    type Style = ();

    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance {
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

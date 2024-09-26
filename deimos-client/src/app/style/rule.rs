use iced::{border::Radius, widget::rule::{self, FillMode}};

use super::Theme;

impl rule::StyleSheet for Theme {
    type Style = ();

    fn appearance(&self, _: &Self::Style) -> rule::Appearance {
        rule::Appearance {
            color: self.rule,
            width: 1,
            radius: Radius::from([0f32, 0f32, 0f32, 0f32]),
            fill_mode: FillMode::Full
        }
    }
}

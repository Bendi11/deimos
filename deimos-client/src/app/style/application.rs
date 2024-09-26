use iced::application;

use super::Theme;

impl application::StyleSheet for Theme {
    type Style = ();

    fn appearance(&self, _: &Self::Style) -> application::Appearance {
        application::Appearance {
            background_color: self.bg_dark,
            text_color: self.text_normal,
        }
    } 
}

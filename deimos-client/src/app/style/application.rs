use iced::application;

use super::Theme;

impl application::DefaultStyle for Theme {
    fn default_style(&self) -> application::Appearance {
        application::Appearance {
            background_color: self.bg_dark,
            text_color: self.text_normal,
        }
    } 
}

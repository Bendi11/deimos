use iced::widget::text;

use super::Theme;


impl text::StyleSheet for Theme {
    type Style = ();

    fn appearance(&self, _: Self::Style) -> text::Appearance {
        text::Appearance {
            color: Some(self.text_normal)
        }
    }
}

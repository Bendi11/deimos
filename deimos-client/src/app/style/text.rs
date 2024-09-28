use iced::widget::text;

use super::Theme;

pub enum TextClass {
    Normal,
    Heading
}

impl text::Catalog for Theme {
    type Class<'a> = TextClass;
    
    fn default<'a>() -> Self::Class<'a> {
        TextClass::Normal
    }

    fn style(&self, item: &Self::Class<'_>) -> text::Style {
        match item {
            TextClass::Normal => text::Style {
                color: Some(self.text_normal)
            },
            TextClass::Heading => text::Style {
                color: Some(self.text_bright)
            }
        }
    }
}

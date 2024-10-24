use super::Theme;
use iced::Background;
use iced_aw::widget::tab_bar;

#[derive(Default)]
pub struct TabsClass {
    pub background: Option<Background>,
}

impl tab_bar::Catalog for Theme {
    type Class<'a> = TabsClass;

    fn default<'a>() -> Self::Class<'a> {
        TabsClass::default()
    }

    fn style(&self, _class: &Self::Class<'_>, _status: tab_bar::Status) -> tab_bar::Style {
        tab_bar::Style::default()
    }
}

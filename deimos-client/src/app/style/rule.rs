use iced::{border::Radius, widget::rule::{self, FillMode}};

use super::Theme;

pub enum RuleClass {
    WhiteLine,
}

impl rule::Catalog for Theme {
    type Class<'a> = RuleClass;

    fn default<'a>() -> Self::Class<'a> {
        RuleClass::WhiteLine
    }

    fn style(&self, class: &Self::Class<'_>) -> rule::Style {
        match class {
            RuleClass::WhiteLine => rule::Style {
                color: self.rule,
                width: 1,
                radius: Radius::from(0f32),
                fill_mode: FillMode::Full
            }
        }
    }
}

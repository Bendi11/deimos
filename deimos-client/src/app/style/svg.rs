use iced::{widget::svg, Color};

use super::Theme;

#[derive(Default)]
pub struct SvgClass {
    normal: Option<Color>,
    hovered: Option<Color>,
}

impl svg::Catalog for Theme {
    type Class<'a> = SvgClass;

    fn default<'a>() -> Self::Class<'a> {
        SvgClass::default()
    }

    fn style(&self, class: &Self::Class<'_>, status: svg::Status) -> svg::Style {
        svg::Style {
            color: match status {
                svg::Status::Idle => class.normal,
                svg::Status::Hovered => class.hovered,
            },
        }
    }
}

impl From<Option<Color>> for SvgClass {
    fn from(value: Option<Color>) -> Self {
        Self {
            normal: value,
            hovered: value
        }
    }
}

impl From<Color> for SvgClass {
    fn from(value: Color) -> Self {
        Self::from(Some(value))
    }
}

impl From<(Color, Color)> for SvgClass {
    fn from(value: (Color, Color)) -> Self {
        Self {
            normal: Some(value.0),
            hovered: Some(value.1),
        }
    }
}

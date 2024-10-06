use iced::{
    border::Radius,
    widget::{
        container,
        scrollable::{self, Scroller},
    },
    Background, Border, Shadow, Vector,
};

use super::Theme;

impl Theme {
    fn scrollbar_container(&self) -> container::Style {
        container::Style {
            text_color: None,
            background: None,
            border: Border {
                width: 0f32,
                color: iced::Color::BLACK,
                radius: Radius::from(0f32),
            },
            shadow: Shadow {
                color: iced::Color::BLACK,
                offset: Vector::new(0f32, 0f32),
                blur_radius: 0f32,
            },
        }
    }

    fn active_scroll_border(&self) -> Border {
        Border {
            color: self.active,
            width: 1f32,
            radius: Radius::from(0.5f32),
        }
    }

    fn invisible_rail(&self) -> scrollable::Rail {
        scrollable::Rail {
            background: None,
            border: Border {
                color: iced::Color::BLACK,
                width: 0f32,
                radius: Radius::from(0f32),
            },
            scroller: Scroller {
                color: self.bg_bright,
                border: Border {
                    color: self.text_normal,
                    width: 1f32,
                    radius: Radius::from(1f32),
                },
            },
        }
    }
}

impl scrollable::Catalog for Theme {
    type Class<'a> = ();

    fn default<'a>() -> Self::Class<'a> {}

    fn style(&self, _: &Self::Class<'_>, status: scrollable::Status) -> scrollable::Style {
        match status {
            scrollable::Status::Active | scrollable::Status::Dragged { .. } => scrollable::Style {
                container: self.scrollbar_container(),
                gap: None,
                vertical_rail: scrollable::Rail {
                    background: Some(Background::Color(self.bg_light)),
                    border: Default::default(),
                    scroller: Scroller {
                        color: self.bg_bright,
                        border: self.active_scroll_border(),
                    },
                },
                horizontal_rail: self.invisible_rail(),
            },
            scrollable::Status::Hovered { .. } => scrollable::Style {
                container: self.scrollbar_container(),
                gap: None,
                vertical_rail: scrollable::Rail {
                    background: Some(Background::Color(self.bg_light)),
                    border: Default::default(),
                    scroller: Scroller {
                        color: self.bg_dark,
                        border: Border {
                            color: self.text_normal,
                            width: 1f32,
                            radius: Radius::from(1f32),
                        },
                    },
                },
                horizontal_rail: self.invisible_rail(),
            },
        }
    }
}

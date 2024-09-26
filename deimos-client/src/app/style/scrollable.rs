use iced::{border::Radius, widget::{container, scrollable}, Border, Shadow, Vector};

use super::Theme;

impl Theme {
    fn scrollbar_container(&self) -> container::Appearance {
        container::Appearance {
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
            }
        }
    }

    fn active_scroll_border(&self) -> Border {
        Border {
            color: self.active,
            width: 1f32,
            radius: Radius::from(0.5f32)
        }
    }
}

impl scrollable::StyleSheet for Theme {
    type Style = ();

    fn active(&self, _: &Self::Style) -> scrollable::Appearance {
        scrollable::Appearance {
            container: self.scrollbar_container(),
            scrollbar: scrollable::Scrollbar {
                background: None,
                border: Border {
                    color: iced::Color::BLACK,
                    width: 0f32,
                    radius: Radius::from(0f32),
                },
                scroller: scrollable::Scroller {
                    color: self.bg_bright,
                    border: self.active_scroll_border(),
                }
            },
            gap: None,
        }
    }

    fn hovered(
            &self,
            _: &Self::Style,
            is_mouse_over_scrollbar: bool,
        ) -> scrollable::Appearance {
        scrollable::Appearance {
            container: self.scrollbar_container(),
            scrollbar: scrollable::Scrollbar {
                background: None,
                border: Border {
                    color: self.active,
                    width: 1f32,
                    radius: Radius::from(0.5f32),
                },
                scroller: scrollable::Scroller {
                    border: match is_mouse_over_scrollbar {
                        true => self.active_scroll_border(),
                        false => Border {
                            color: iced::Color::BLACK,
                            width: 0f32,
                            radius: Radius::default(),
                        }
                    },
                    color: self.bg_light,
                }
            },
            gap: None,
        }
    }
}

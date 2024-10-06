use iced::Color;


pub mod orbit;

pub mod application;
pub mod text;
pub mod rule;
pub mod scrollable;
pub mod button;
pub mod container;
pub mod svg;
pub mod text_input;

pub struct Theme {
    pub bg_dark: Color,
    pub bg_light: Color,
    pub bg_bright: Color,
    pub rule: Color,

    pub text_bright: Color,
    pub text_normal: Color,
    pub text_dim: Color,
    
    pub active: Color,
    pub warn: Color,
    pub error: Color,
}

impl Theme {
    /// Create a default theme with orbit colorscheme colors
    pub const fn new() -> Self {
        Self {
            bg_dark: orbit::NIGHT[2],
            bg_light: orbit::NIGHT[1],
            bg_bright: orbit::NIGHT[0],
            rule: orbit::MERCURY[0],
            text_bright: orbit::SOL[0],
            text_normal: orbit::MERCURY[1],
            text_dim: orbit::MERCURY[3],
            active: orbit::EARTH[2],
            warn: orbit::MARS[1],
            error: orbit::MARS[3],
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

pub type Element<'a, T> = iced::Element<'a, T, Theme, iced::Renderer>;
pub type Row<'a, T> = iced::widget::Row<'a, T, Theme, iced::Renderer>;
pub type Column<'a, T> = iced::widget::Column<'a, T, Theme, iced::Renderer>;
pub type Text<'a> = iced::widget::Text<'a, Theme, iced::Renderer>;
pub type TextInput<'a, T> = iced::widget::TextInput<'a, T, Theme, iced::Renderer>;
pub type Rule<'a> = iced::widget::Rule<'a, Theme>;
pub type Container<'a, T> = iced::widget::Container<'a, T, Theme, iced::Renderer>;
pub type Scrollable<'a, T> = iced::widget::Scrollable<'a, T, Theme, iced::Renderer>;
pub type Button<'a, T> = iced::widget::Button<'a, T, Theme, iced::Renderer>;
pub type Svg<'a> = iced::widget::Svg<'a, Theme>;

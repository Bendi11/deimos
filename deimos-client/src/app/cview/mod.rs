use iced::{Length, Padding};

use crate::context::{Context, PodRef};

use super::style::{Column, Element, Rule, Text};

#[derive(Debug, Clone)]
pub struct PodView {
    viewed: Option<PodRef>,
}

#[derive(Debug, Clone)]
pub enum PodViewMessage {
    ChangeView(PodRef),
}

impl PodView {
    pub fn new() -> Self {
        Self { viewed: None }
    }

    pub fn view<'a>(&self, ctx: &'a Context) -> Element<'a, PodViewMessage> {
        let Some(viewed) = self.viewed.and_then(|k| ctx.pods.get(k)) else {
            return Text::new("").into();
        };

        Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::new(8f32).top(32f32))
            .push(Text::new(&viewed.data.name).size(22))
            .push(Rule::horizontal(1))
            .into()
    }

    pub fn update(&mut self, ctx: &mut Context, msg: PodViewMessage) -> iced::Task<PodViewMessage> {
        match msg {
            PodViewMessage::ChangeView(c) => {
                self.viewed = Some(c);
                iced::Task::none()
            }
        }
    }
}

use std::sync::Weak;

use iced::{Length, Padding};

use crate::context::{container::CachedContainer, ContainerRef, Context};

use super::style::{orbit, Column, Element, Rule, Text};

#[derive(Debug, Clone)]
pub struct ContainerView {
    viewed: Option<ContainerRef>,
}

#[derive(Debug, Clone)]
pub enum ContainerViewMessage {
    ChangeView(ContainerRef),
}

impl ContainerView {
    pub fn new() -> Self {
        Self { viewed: None }
    }

    pub fn view<'a>(&self, ctx: &'a Context) -> Element<'a, ContainerViewMessage> {
        let Some(viewed) = self.viewed.and_then(|k| ctx.containers.get(k)) else {
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

    pub fn update(
        &mut self,
        ctx: &mut Context,
        msg: ContainerViewMessage,
    ) -> iced::Task<ContainerViewMessage> {
        match msg {
            ContainerViewMessage::ChangeView(c) => {
                self.viewed = Some(c);
                iced::Task::none()
            }
        }
    }
}

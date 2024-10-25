use iced::{Length, Padding};
use logs::{PodLogsMessage, PodLogsView};

use crate::context::{Context, PodRef};

use super::style::{Button, Column, Container, Element, Text};

mod logs;

#[derive(Debug)]
pub struct PodView {
    viewed: Option<PodRef>,
    logs: PodLogsView,
    tab: PodTab,
}

#[derive(Debug, Clone)]
pub enum PodViewMessage {
    Closed,
    ViewPod(PodRef),
    Tab(PodTab),
    Logs(PodLogsMessage),
}

#[derive(Debug, Clone, Copy)]
pub enum PodTab {
    Overview,
    Logs,
}

impl PodView {
    pub fn new() -> Self {
        Self {
            viewed: None,
            logs: PodLogsView::new(),
            tab: PodTab::Overview
        }
    }

    pub fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, PodViewMessage> {
        let Some(viewed) = self.viewed.and_then(|k| ctx.pods.get(k)) else {
            return Text::new("").into();
        };

        let tab_view = match self.tab {
            PodTab::Overview => Text::new("Overview").into(),
            PodTab::Logs => self.logs.view(ctx).map(PodViewMessage::Logs)
        };
        
        Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::new(8f32).top(32f32))
            .push(Text::new(&viewed.data.name).size(28))
            .push(
                Button::new("Logs")
                    .on_press(PodViewMessage::Tab(PodTab::Logs))
            )
            .push(
                Container::new(tab_view)
                    .padding(Padding::new(0f32).left(20f32).right(20f32).top(40f32).bottom(20f32))
            )
            .into()
    }

    pub fn update(&mut self, ctx: &mut Context, msg: PodViewMessage) -> iced::Task<PodViewMessage> {
        match msg {
            PodViewMessage::Closed => {
                let task = match self.tab {
                    PodTab::Logs => self.logs.update(ctx, self.viewed.unwrap(), PodLogsMessage::Close).map(PodViewMessage::Logs),
                    _ => iced::Task::none()
                };
                self.viewed = None;
                task
            },
            PodViewMessage::ViewPod(c) => {
                self.viewed = Some(c);
                iced::Task::none()
            },
            PodViewMessage::Tab(tab) => {
                self.tab = tab;
                match self.tab {
                    PodTab::Logs => self.logs.update(ctx, self.viewed.unwrap(), PodLogsMessage::Open).map(PodViewMessage::Logs),
                    _ => iced::Task::none()
                }
            },
            PodViewMessage::Logs(msg) => match (self.tab, self.viewed) {
                (PodTab::Logs, Some(viewed)) => self.logs.update(ctx, viewed, msg).map(PodViewMessage::Logs),
                _ => iced::Task::none()
            },
        }
    }
}

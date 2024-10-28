use iced::{Length, Padding};
use logs::{PodLogsMessage, PodLogsView};

use crate::context::{Context, PodRef};

use super::style::{Button, Column, Container, Element, Row, Text};

mod logs;

#[derive(Debug)]
pub struct PodView {
    viewed: Option<PodRef>,
    tab: PodTab,
}

#[derive(Debug, Clone)]
pub enum PodViewMessage {
    Closed,
    ViewPod(PodRef),
    Tab(PodTab),
    Logs(PodLogsMessage),
}

#[derive(Debug, Clone)]
pub enum PodTab {
    Overview,
    Logs(PodLogsView),
}

impl PodView {
    pub fn new() -> Self {
        Self {
            viewed: None,
            tab: PodTab::Overview
        }
    }

    pub fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, PodViewMessage> {
        let Some(viewed) = self.viewed.and_then(|k| ctx.pods.get(k)) else {
            return Text::new("").into();
        };

        let tab_view = match self.tab {
            PodTab::Overview => Text::new("Overview").into(),
            PodTab::Logs(ref logs) => logs.view(ctx).map(PodViewMessage::Logs)
        };

        let tabbar = Row::new()
            .width(Length::Fill)
            .spacing(4)
            .push(
                Button::new(Text::new("Logs").size(16))
                    .on_press_with(|| PodViewMessage::Tab(PodTab::Logs(PodLogsView::default())))
                    .width(Length::FillPortion(1))
            );
        
        Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::new(8f32).top(32f32))
            .push(Text::new(&viewed.data.name).size(28))
            .push(tabbar)
            .push(
                Container::new(tab_view)
                    .padding(Padding::new(0f32).left(20f32).right(20f32).top(40f32).bottom(20f32))
            )
            .into()
    }

    pub fn update(&mut self, ctx: &mut Context, msg: PodViewMessage) -> iced::Task<PodViewMessage> {
        match msg {
            PodViewMessage::Closed => {
                self.viewed = None;
                self.tab = PodTab::Overview;
                iced::Task::none()
            },
            PodViewMessage::ViewPod(c) => {
                self.viewed = Some(c);
                self.tab = PodTab::Overview;
                iced::Task::none()
            },
            PodViewMessage::Tab(tab) => {
                self.tab = tab;
                iced::Task::none()
            },
            PodViewMessage::Logs(msg) => match (&mut self.tab, self.viewed) {
                (PodTab::Logs(..), None) => {
                    tracing::error!("FUCK");
                    iced::Task::none()
                },
                (PodTab::Logs(ref mut logs), Some(viewed)) => logs.update(ctx, viewed, msg).map(PodViewMessage::Logs),
                _ => iced::Task::none()
            },
        }
    }
}

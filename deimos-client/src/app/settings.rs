use std::sync::Arc;

use fltk::{button::Button, enums::{Align, Font, FrameType}, frame::Frame, group::{Flex, Pack, PackType}, image::SvgImage, input::Input, prelude::{GroupExt, InputExt, WidgetBase, WidgetExt}};

use crate::context::Context;

use super::{orbit, widget};


pub struct Settings {
    column: Flex,
    host_url: Input,
}

impl Settings {
    pub fn new<P: GroupExt>(ctx: Arc<Context>, parent: &mut P) -> Self {
        let mut column = Flex::default()
            .column()
            .with_size(parent.width() - 32, parent.height())
            .center_of(parent);
        column.end();
        column.set_color(orbit::NIGHT[2]);
        parent.add(&column);

        let mut top_bar = Pack::default();
        top_bar.set_size(column.width(), 42);
        top_bar.end();

        let save = SvgImage::from_data(include_str!("../../assets/check.svg")).unwrap();
        let save_img = widget::svg::svg_color(save, top_bar.height(), orbit::SOL[1]);
        let mut save_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        save_button.set_size(top_bar.height(), top_bar.height());
        save_button.set_image_scaled(Some(save_img));
        save_button.visible_focus(false);
        top_bar.add(&save_button);

        column.add(&top_bar);
        column.fixed(&top_bar, 42);
       
        let mut frame = Frame::default();
        frame.set_align(Align::Inside | Align::Left);
        frame.set_label_color(orbit::SOL[0]);
        frame.set_label("Host URL");
        frame.set_label_size(18);
        //frame.set_align(Align::Left);
        column.add(&frame);
        column.fixed(&frame, 32);
    
        let mut host_url = Input::default();
        host_url.set_frame(fltk::enums::FrameType::RShadowBox);
        host_url.set_text_color(orbit::MERCURY[1]);
        host_url.set_text_font(Font::Courier);
        host_url.set_text_size(18);
        host_url.set_cursor_color(orbit::SOL[0]);
        host_url.set_color(orbit::NIGHT[1]);
        host_url.set_label_color(orbit::SOL[0]);
        
        column.add(&host_url);
        column.fixed(&host_url, 40);

        Self {
            column,
            host_url,
        }
    }

    pub const fn group(&self) -> &impl GroupExt {
        &self.column
    }

    pub fn group_mut(&mut self) -> &mut impl GroupExt {
        &mut self.column
    }
}

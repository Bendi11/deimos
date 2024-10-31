use fltk::{enums::{Color, Font}, frame::Frame, group::Flex, image::{RgbImage, SvgImage}, prelude::{GroupExt, ImageExt, WidgetExt}};

use super::{orbit, widget};



pub struct Header {
    row: Flex,
}

impl Header {
    pub fn create<P: GroupExt>(parent: &mut P) -> Self {
        let mut row = Flex::default()
            .row()
            .with_size(parent.width(), parent.height() / 7);
        row.end();
        parent.add(&row);

        let deimos_icon = SvgImage::from_data(include_str!("../../assets/mars-deimos.svg"))
            .unwrap();

        let deimos_rgb = widget::svg::svg_color(deimos_icon, row.height(), orbit::MARS[2]);
        let mut frame = Frame::default();
        frame.set_size(row.height(), row.height());
        frame.set_image(Some(deimos_rgb));
        row.add(&frame);

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(Color::from_rgb(0xff, 0xf4, 0xea));
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);

        row.add(&title_frame);

        Self {
            row
        }
    }

    pub const fn group(&self) -> &impl GroupExt {
        &self.row
    }

    pub fn group_mut(&mut self) -> &mut impl GroupExt {
        &mut self.row
    }
}

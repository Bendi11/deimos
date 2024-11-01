use fltk::{button::Button, enums::{Color, Font}, frame::Frame, group::{Flex, Group}, image::{RgbImage, SvgImage}, prelude::{GroupExt, ImageExt, WidgetExt}};

use super::{orbit, widget, DeimosStateHandle};



pub struct Header {
    top: Group,
    row: Flex,
}

impl Header {
    pub fn create<P: GroupExt>(state: DeimosStateHandle, parent: &mut P) -> Self {
        let mut top = Group::default()
            .with_size(parent.width(), parent.height());
        top.end();
        parent.add_resizable(&top);

        let mut row = Flex::default()
            .row()
            .with_size(top.width(), top.height() / 7);
        row.end();
        row.set_margins(8, 16, 8, 8);
        top.add(&row);

        let deimos_icon = SvgImage::from_data(include_str!("../../assets/mars-deimos.svg"))
            .unwrap();

        let deimos_rgb = widget::svg::svg_color(deimos_icon, row.height(), orbit::MARS[2]);
        let mut frame = Frame::default();
        frame.set_size(row.height(), row.height());
        frame.set_image(Some(deimos_rgb));
        row.add(&frame);

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(orbit::SOL[0]);
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);
        row.add(&title_frame);

        let settings_icon = SvgImage::from_data(include_str!("../../assets/settings.svg")).unwrap();
        let settings_rgb = widget::svg::svg_color(settings_icon, row.height() / 2, orbit::MERCURY[2]);
        let mut settings_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        settings_button.set_image(Some(settings_rgb));
        settings_button.set_callback(move |_| {
            let state = state.clone();
            tokio::spawn(async move {
                let mut lock = state.active.lock().await;
                lock.hide();
                *lock = state.settings.group().clone();
                lock.show();
            });
        });

        row.add(&settings_button);
        row.fixed(&settings_button, row.height() / 2);

        Self {
            top,
            row
        }
    }

    pub const fn group(&self) -> &Group {
        &self.top
    }

    pub fn group_mut(&mut self) -> &mut Group {
        &mut self.top
    }
}

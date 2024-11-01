use fltk::{enums::Font, frame::Frame, group::{Flex, Group}, image::SvgImage, prelude::{GroupExt, WidgetExt}};

use crate::app::{orbit, widget, DeimosStateHandle};

use super::Overview;


impl Overview {
    pub fn header<P: GroupExt>(state: DeimosStateHandle, parent: &P) -> Flex {
        let mut row = Flex::default()
            .with_size(parent.width(), parent.height() / 7)
            .row();
        row.end();
        row.set_margins(32, 16, 8, 16);

        let deimos_icon = SvgImage::from_data(include_str!("../../../assets/mars-deimos.svg"))
            .unwrap();

        let deimos_rgb = widget::svg::svg_color(deimos_icon, row.height(), orbit::MARS[2]);
        let mut frame = Frame::default();
        frame.set_size(row.height(), row.height());
        frame.set_image_scaled(Some(deimos_rgb));
        row.add(&frame);
        row.fixed(&frame, row.height());

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(orbit::SOL[0]);
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);
        row.add(&title_frame);

        let settings_icon = SvgImage::from_data(include_str!("../../../assets/settings.svg")).unwrap();
        let settings_rgb = widget::svg::svg_color(settings_icon, row.height() / 2, orbit::MERCURY[2]);
        let mut settings_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        settings_button.set_image(Some(settings_rgb));
        settings_button.set_callback(move |_| state.clone().set_view(state.settings.group()));

        row.add(&settings_button);
        row.fixed(&settings_button, row.height() / 2);

        row
    }
}

use fltk::{enums::{Align, Font}, frame::Frame, group::{Flex, Group, Pack, PackType}, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::app::{orbit, widget, DeimosStateHandle};

use super::Overview;


impl Overview {
    pub fn header(state: DeimosStateHandle) -> impl GroupExt {
        let mut row = Flex::default_fill()
            .row()
            .with_align(Align::Center);
        row.set_margins(8, 0, 0, 8);

        let deimos_icon = SvgImage::from_data(include_str!("../../../assets/mars-deimos.svg"))
            .unwrap();
        let deimos_rgb = widget::svg::svg_color(deimos_icon, 128, orbit::MARS[2]);
        let mut frame = Frame::default().with_size(64, 64);
        frame.resize_callback(widget::svg::resize_image_cb(0, 0));
        frame.set_image(Some(deimos_rgb));

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(orbit::SOL[0]);
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);

        let settings_icon = SvgImage::from_data(include_str!("../../../assets/settings.svg")).unwrap();
        let settings_rgb = widget::svg::svg_color(settings_icon, 128, orbit::MERCURY[2]);
        let mut settings_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        settings_button.set_image(Some(settings_rgb));
        settings_button.set_callback(move |_| {
            let state = state.clone();
            tokio::spawn(
                async move {
                    state.set_view(state.settings.group()).await;
                }
            );
        });
        settings_button.resize_callback(widget::svg::resize_image_cb(0, 0));

        row.fixed(&settings_button, row.height());
        row.resize_callback(move |r,_,_,_,_| {
            r.fixed(&settings_button, r.height());
            r.fixed(&frame, r.height());
        });
        

        row.end();
        row
    }
}

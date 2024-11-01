use fltk::{enums::{Align, Font}, frame::Frame, group::{Flex, Group, Pack, PackType}, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::app::{orbit, widget, DeimosStateHandle};

use super::Overview;


impl Overview {
    pub fn header(state: DeimosStateHandle) -> impl GroupExt {
        

        let mut row = Flex::default_fill()
            .row()
            .with_align(Align::Center);
        row.end();
        row.set_margins(8, 0, 0, 8);

        let deimos_icon = SvgImage::from_data(include_str!("../../../assets/mars-deimos.svg"))
            .unwrap();
        let deimos_rgb = widget::svg::svg_color(deimos_icon, 128, orbit::MARS[2]);
        let mut frame = Frame::default().with_size(64, 64);
        frame.resize_callback(|f, _, _, _, _| {
            if let Some(mut image) = f.image() {
                image.scale(f.height(), f.height(), true, true);
                f.redraw();
            }
        });
        frame.set_image(Some(deimos_rgb));
        row.add(&frame);

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(orbit::SOL[0]);
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);
        row.add(&title_frame);

        let settings_icon = SvgImage::from_data(include_str!("../../../assets/settings.svg")).unwrap();
        let settings_rgb = widget::svg::svg_color(settings_icon, 128, orbit::MERCURY[2]);
        let mut settings_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        settings_button.set_image(Some(settings_rgb));
        settings_button.set_callback(move |_| state.clone().set_view(state.settings.group()));
        settings_button.resize_callback(
            |f,_,_,_,_| {
                if let Some(mut image) = f.image() {
                    image.scale(f.height(), f.height(), true, true);
                    f.redraw();
                }
            }
        );

        row.add(&settings_button);
        row.fixed(&settings_button, row.height());
        row.resize_callback(move |r,_,_,_,_| {
            //settings_button.set_size(r.height() - 16, r.height() - 16);
            r.fixed(&settings_button, r.height() - 16);
            r.fixed(&frame, r.height());
        });

        row
    }
}

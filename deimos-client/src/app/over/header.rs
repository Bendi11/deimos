use fltk::{enums::{Align, Font}, frame::Frame, group::Flex, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::{app::{orbit, widget, DeimosStateHandle}, context::ContextConnectionState};

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
        
        {
            let mut title_col = Flex::default_fill().column();
            let mut title_frame = Frame::default()
                .with_label("Deimos");
            title_frame.set_label_color(orbit::SOL[0]);
            title_frame.set_label_font(Font::CourierBold);
            title_frame.set_label_size(32);
            title_col.fixed(&title_frame, 32);

            let mut connection_status = Frame::default();
            connection_status.set_label_font(Font::Screen);
            connection_status.set_label_size(12);
            title_col.fixed(&connection_status, 12);
            
            let state = state.clone();
            tokio::task::spawn(
                async move {
                    let mut sub = state.ctx.conn.subscribe();
                    loop {
                        {
                            let state = sub.borrow_and_update();

                            fltk::app::lock().ok();
                            match *state {
                                ContextConnectionState::Unknown => {
                                    connection_status.set_label("Connecting");
                                    connection_status.set_label_color(orbit::MERCURY[2]);
                                },
                                ContextConnectionState::Connected => {
                                    connection_status.set_label("Connected");
                                    connection_status.set_label_color(orbit::EARTH[0]);
                                },
                                ContextConnectionState::Error => {
                                    connection_status.set_label("Disconnected");
                                    connection_status.set_label_color(orbit::MARS[1]);
                                }
                            }

                            connection_status.set_damage(true);

                            fltk::app::unlock();
                            fltk::app::awake();
                        }

                        if sub.changed().await.is_err() {
                            break
                        }
                    }
                }
            );

            title_col.end();
        }

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
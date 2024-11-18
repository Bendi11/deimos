use fltk::{button::Button, enums::Align, frame::Frame, group::Flex, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::{app::{orbit, widget, DeimosStateHandle}, context::client::ContextConnectionState};


pub fn header(state: DeimosStateHandle) -> impl GroupExt {
    let mut row = Flex::default()
        .with_size(0, 64)
        .row()
        .with_align(Align::Center);
    row.set_margins(8, 0, 0, 8);

    let deimos_icon = SvgImage::from_data(include_str!("../../../assets/mars-deimos.svg"))
        .unwrap();
    let deimos_rgb = widget::svg::svg_color(deimos_icon, 64, orbit::MARS[2]);
    let mut frame = Frame::default().with_size(64, 64);
    frame.set_image(Some(deimos_rgb));
    
    {
        let mut title_col = Flex::default().column();
        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(orbit::SOL[0]);
        title_frame.set_label_font(crate::app::HEADER_FONT);
        title_frame.set_label_size(32);
        title_col.fixed(&title_frame, 34);

        let mut connection_status = Frame::default();
        connection_status.set_label_font(crate::app::GENERAL_FONT);
        connection_status.set_label_size(10);
        title_col.fixed(&connection_status, 16);
        
        let state = state.clone();
        tokio::task::spawn(
            async move {
                let mut sub = state.ctx.clients.conn.subscribe();
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
                            },
                            ContextConnectionState::NoToken => {
                                connection_status.set_label("No Authorization Token");
                                connection_status.set_label_color(orbit::VENUS[1]);
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
    
    let icon_size = row.height() - 16;
    let authentication_icon = SvgImage::from_data(include_str!("../../../assets/key.svg")).unwrap();
    let authentication_grey = widget::svg::svg_color(authentication_icon.clone(), icon_size, orbit::MERCURY[2]);
    let authentication_red  = widget::svg::svg_color(authentication_icon, icon_size, orbit::MARS[2]);
    let mut authentication_button = widget::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    {
        let state = state.clone();
        authentication_button.set_callback(move |_| {
            let state = state.clone();
            tokio::task::spawn(async move {
                state.set_view(state.authorization.clone()).await;
            });
        });
    }

    {
        let state = state.clone();
        let mut authentication_button = authentication_button.clone();
        tokio::task::spawn(
            async move {
                let mut sub = state.ctx.clients.token.subscribe();
                loop {
                    {
                        let token = sub.borrow_and_update();

                        fltk::app::lock().ok();

                        authentication_button.set_image(
                            Some(
                                if token.token().is_some() { authentication_grey.clone() } else { authentication_red.clone() }
                            )
                        );
                        authentication_button.set_damage(true);

                        fltk::app::unlock();
                        fltk::app::awake();
                    }

                    if sub.changed().await.is_err() {
                        break
                    }
                }
            }
        );
    }

    let settings_icon = SvgImage::from_data(include_str!("../../../assets/settings.svg")).unwrap();
    let settings_rgb = widget::svg::svg_color(settings_icon, row.height() - 16, orbit::MERCURY[2]);
    let mut settings_button = widget::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    settings_button.set_image(Some(settings_rgb));
    settings_button.set_callback(move |_| {
        let state = state.clone();
        tokio::spawn(
            async move {
                state.set_view(state.settings.clone()).await;
            }
        );
    });

    row.fixed(&settings_button, row.height());
    row.resize_callback(move |r,_,_,_,_| {
        r.fixed(&settings_button, r.height());
        r.fixed(&frame, r.height());
        r.fixed(&authentication_button, r.height());
    });
    

    row.end();
    row
}

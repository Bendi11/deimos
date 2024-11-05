use std::{str::FromStr, time::Duration};

use fltk::{dialog::NativeFileChooser, enums::{Align, CallbackTrigger, Font}, frame::Frame, group::{Flex, Group, Pack}, image::SvgImage, input::{Input, IntInput}, prelude::{GroupExt, InputExt, WidgetBase, WidgetExt}};
use http::Uri;

use crate::context::ContextSettings;

use super::{orbit, widget, DeimosStateHandle};


pub struct Settings {
    top: Group,
}

impl Settings {
    pub fn new(state: DeimosStateHandle) -> Self {
        let mut top = Group::default_fill();
        top.hide();
        
        let mut column = Flex::default()
            .column()
            .with_size(top.width() - 32, top.height())
            .center_of(&top);
        column.set_color(orbit::NIGHT[2]);
        column.set_spacing(32);
        
        let mut save_button = {
            let top_bar = Pack::default_fill();
            column.fixed(&top_bar, 42);

            let save = SvgImage::from_data(include_str!("../../assets/check.svg")).unwrap();
            let save_img = widget::svg::svg_color(save, top_bar.height(), orbit::SOL[1]);
            let mut save_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
            save_button.set_size(top_bar.height(), top_bar.height());
            save_button.set_image_scaled(Some(save_img));
            save_button.resize_callback(widget::svg::resize_image_cb(0, 0));

            save_button
        };
       

    
        let mut host_url = Self::input_box::<Input>(&mut column, "Host URL");
        let mut request_timeout = Self::input_box::<IntInput>(&mut column, "gRPC Request Timeout (seconds)");
        let mut connect_timeout = Self::input_box::<IntInput>(&mut column, "gRPC Connection Timeout (seconds)");

        let mut file_select = NativeFileChooser::new(fltk::dialog::FileDialogType::BrowseFile);

        Self::input_lbl(&mut column, "SSL Certificate Path");
        let mut cert_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        column.fixed(&cert_button, 32);
        cert_button.set_label("Select");
        cert_button.set_label_font(Font::Screen);
        cert_button.set_callback(
            move |_| {
                file_select.show();
            }
        );
         
        
        {
            let state = state.clone();
            let mut host_url = host_url.clone();
            let mut request_timeout = request_timeout.clone();
            let mut connect_timeout = connect_timeout.clone();
            tokio::task::spawn(
                async move {
                    let mut sub = state.ctx.state.settings.subscribe();
                    loop {
                        let Ok(_) = sub.changed().await else {
                            break
                        };
                        
                        let settings = sub.borrow_and_update();
                        
                        fltk::app::lock().ok();

                        host_url.set_value(&settings.server_uri.to_string());
                        request_timeout.set_value(&settings.request_timeout.as_secs().to_string());
                        connect_timeout.set_value(&settings.connect_timeout.as_secs().to_string());

                        fltk::app::unlock();
                    }
                }
            );
        }

        top.end();
    
        save_button.set_callback(move |_| {
            fltk::app::lock().ok();
            
            fn parse_from<T, I: InputExt, M: FnOnce(String) -> Option<T>>(input: &mut I, map: M) -> Option<T> {
                match map(input.value()) {
                    None => {
                        input.set_text_color(orbit::MARS[1]);
                        input.redraw();
                        None
                    },
                    val => val,
                }
            }

            let server_uri = parse_from(&mut host_url, |val| Uri::from_str(&val).ok());
            let request_timeout = parse_from(&mut request_timeout, |val| u64::from_str(&val).ok().map(Duration::from_secs));
            let connect_timeout = parse_from(&mut connect_timeout, |val| u64::from_str(&val).ok().map(Duration::from_secs));
            
            fltk::app::unlock();
            fltk::app::awake();

            let (
                Some(server_uri),
                Some(request_timeout),
                Some(connect_timeout)
            ) = (server_uri, request_timeout, connect_timeout) else {
                return
            };
            
            let settings =  ContextSettings {
                server_uri,
                request_timeout,
                connect_timeout,
                ..Default::default()
            };

            tracing::trace!("Got new settings {:?}", settings);
            
            let state = state.clone();
            tokio::task::spawn(
                async move {
                    state.set_view(state.overview.group()).await;
                    state.ctx.reload(settings).await;
                }
            );
        });

        Self {
            top,
        }
    }

    pub fn group(&self) -> Group {
        self.top.clone()
    }

    fn input_lbl(column: &mut Flex, label: &str) -> Frame {
        let mut host_lbl = Frame::default();
        column.fixed(&host_lbl, 32);
        host_lbl.set_align(Align::Inside | Align::Left);
        host_lbl.set_label_color(orbit::SOL[0]);
        host_lbl.set_label(label);
        host_lbl.set_label_size(18);
        host_lbl
    }

    fn input_box<I: InputExt + Default>(column: &mut Flex, label: &str) -> I {
        Self::input_lbl(column, label);

        let mut input = I::default();
        column.fixed(&input, 40);
        input.set_frame(fltk::enums::FrameType::RShadowBox);
        input.set_text_color(orbit::MERCURY[1]);
        input.set_text_font(Font::Courier);
        input.set_text_size(18);
        input.set_cursor_color(orbit::SOL[0]);
        input.set_color(orbit::NIGHT[1]);
        input.set_label_color(orbit::SOL[0]);
        input.set_trigger(CallbackTrigger::Changed);
        input.set_callback(|i| {
            if i.text_color() != orbit::MERCURY[1] {
                i.set_text_color(orbit::MERCURY[1]);
                i.redraw();
            }
        });

        input
    }
}

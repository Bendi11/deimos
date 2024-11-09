use std::{str::FromStr, time::Duration};

use fltk::{enums::Align, group::{Group, Pack, PackType}, image::SvgImage, input::{Input, IntInput}, prelude::{GroupExt, InputExt, WidgetBase, WidgetExt}};
use http::Uri;

use crate::context::client::{auth::PersistentTokenKind, ContextSettings};

use super::{orbit, widget::{self, input::input_box}, DeimosStateHandle};


pub fn settings(state: DeimosStateHandle) -> Group {
    let mut top = Pack::default_fill();
    top.set_size(top.width() - 16, top.height());
    top.set_pos(8, 0);
    top.set_type(PackType::Vertical);
    top.set_spacing(8);
    top.set_color(orbit::NIGHT[2]);
    top.hide();
    
    /*let mut column = Flex::default()
        .column()
        .with_size(top.width() - 32, top.height())
        .center_of(&top);
    column.set_color(orbit::NIGHT[2]);
    column.set_spacing(32);*/
    
    let mut save_button = {
        //let top_bar = Pack::default().with_size(top.width(), 42);
        //column.fixed(&top_bar, 42);

        let save = SvgImage::from_data(include_str!("../../assets/check.svg")).unwrap();
        let save_img = widget::svg::svg_color(save, 42, orbit::SOL[1]);
        let mut save_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        save_button.set_size(42, 42);
        save_button.set_image_scaled(Some(save_img));
        save_button.resize_callback(widget::svg::resize_image_cb(0, 0));

        save_button
    };
   


    let (frame, mut host_url) = input_box::<Input>("Host URL");
    frame.center_of_parent().with_size(top.width() - 16, 60);
    let (frame, mut request_timeout) = input_box::<IntInput>("gRPC Request Timeout (seconds)");
    frame.center_of_parent().with_size(top.width() - 16, 60);
    let (frame, mut connect_timeout) = input_box::<IntInput>("gRPC Connection Timeout (seconds)");
    frame.with_size(top.width() - 16, 60);

    {
        let state = state.clone();
        let mut host_url = host_url.clone();
        let mut request_timeout = request_timeout.clone();
        let mut connect_timeout = connect_timeout.clone();
        tokio::task::spawn(
            async move {
                let mut sub = state.ctx.clients.settings.subscribe();
                loop {
                    {
                        let settings = sub.borrow_and_update();
                        
                        fltk::app::lock().ok();

                        host_url.set_value(&settings.server_uri.to_string());
                        request_timeout.set_value(&settings.request_timeout.as_secs().to_string());
                        connect_timeout.set_value(&settings.connect_timeout.as_secs().to_string());

                        fltk::app::unlock();
                    }

                    let Ok(_) = sub.changed().await else {
                        break
                    };
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
        
        let settings = ContextSettings {
            server_uri,
            request_timeout,
            connect_timeout,
            token_protect: PersistentTokenKind::Plaintext,
        };

        tracing::trace!("Got new settings {:?}", settings);
        
        let state = state.clone();
        tokio::task::spawn(
            async move {
                state.set_view(state.overview.clone()).await;
                state.ctx.clients.reload(settings).await;
            }
        );
    });
    
    top.as_group().unwrap()
}

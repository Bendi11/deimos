use fltk::{enums::{Font, FrameType}, frame::Frame, group::{Flex, Group, Scroll}, image::SvgImage, prelude::{GroupExt, WidgetExt}};

use crate::context::pod::CachedPod;

use super::{orbit, widget, DeimosStateHandle};

pub mod header;


pub struct Overview {
    top: Group,
}


impl Overview {
    pub fn new<P: GroupExt>(state: DeimosStateHandle, parent: &P) -> Self {
        let mut top = Group::default();
        top.set_size(parent.width(), parent.height());
        top.end();
        top.hide();

        let header = Self::header(state.clone(), &top);
        top.add(&header);

        let mut pods_scroll = Scroll::default();
        pods_scroll.end();
        pods_scroll.set_size(parent.width(), 1000);
        top.add(&pods_scroll);

        Self {
            top,
        }
    }
    
    /// Create a button with a brief overview of the given pod
    pub fn pod_button(pod: &CachedPod, width: i32) -> impl GroupExt {
        let mut row = Flex::default().row().with_size(width, 64);
        row.end();
        row.set_frame(FrameType::RShadowBox);
        row.set_color(orbit::NIGHT[1]);

        let mut data = Frame::default();
        data.set_label(&pod.data.name);
        data.set_label_font(Font::CourierBold);
        data.set_label_color(orbit::SOL[1]);
        
        row.add(&data);
        row.fixed(&data, width * 2 / 3);
        
        let start_svg = SvgImage::from_data(include_str!("../../../assets/start.svg")).unwrap();
        let start_rgb = widget::svg::svg_color(start_svg, width / 2, orbit::MERCURY[1]);
        let mut button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[3]);
        button.set_image_scaled(Some(start_rgb));
        row.add(&button); 

        row
    }

    pub fn group(&self) -> Group {
        self.top.clone()
    }
}

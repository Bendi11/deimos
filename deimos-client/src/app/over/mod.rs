use fltk::{enums::{Align, Font, FrameType}, frame::Frame, group::{Flex, Group, Pack, PackType, Scroll, ScrollType}, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::context::pod::CachedPod;

use super::{orbit, widget, DeimosStateHandle};

pub mod header;


pub struct Overview {
    top: Group,
}


impl Overview {
    pub fn new(state: DeimosStateHandle) -> Self {
        let mut top = {
            let top = Group::default_fill();
            let mut flex = Flex::default_fill().column();
            {
                let header = Self::header(state.clone());
                flex.fixed(&header, 64);

                {

                    {
                        let mut scroll = Scroll::default_fill();
                        top.resizable(&scroll);
                        scroll.set_frame(FrameType::NoBox);
                        scroll.set_color(orbit::NIGHT[1]);
                        scroll.set_type(ScrollType::Vertical);
                        
                        let mut pods_pack = Pack::default_fill();
                        scroll.resizable(&pods_pack);
                        pods_pack.set_spacing(32);
                        pods_pack.set_frame(FrameType::NoBox);
                        pods_pack.set_color(orbit::SOL[0]);
                        pods_pack.set_type(PackType::Vertical);

                        let mut top2 = top.clone();
                        tokio::spawn(
                            async move {
                                for (i, pod) in state.ctx.pods.values().enumerate() {
                                    let button = Self::pod_button(pod);
                                    pods_pack.add(&button);  
                                }
                                top2.redraw();
                            }
                        );
                    }
                }
            }
            top
        };

        top.hide();
        
        Self {
            top,
        }
    }
    
    /// Create a button with a brief overview of the given pod
    pub fn pod_button(pod: &CachedPod) -> impl GroupExt {
        let mut row = Flex::new(0, 0, 0, 64, "").row();
        row.end();
        row.set_frame(FrameType::RShadowBox);
        row.set_color(orbit::NIGHT[1]);

        let mut data = Frame::default();
        data.set_label(&pod.data.name);
        data.set_label_font(Font::CourierBold);
        data.set_label_color(orbit::SOL[1]);
        
        row.add(&data);
        
        let start_svg = SvgImage::from_data(include_str!("../../../assets/start.svg")).unwrap();
        let start_rgb = widget::svg::svg_color(start_svg, 128, orbit::MERCURY[1]);
        let mut button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[3]);
        button.set_image_scaled(Some(start_rgb));
        row.add(&button); 

        row
    }

    pub fn group(&self) -> Group {
        self.top.clone()
    }
}

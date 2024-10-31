use fltk::{enums::{Color, ColorDepth, Font}, frame::Frame, group::Flex, image::{RgbImage, SvgImage}, prelude::{GroupExt, ImageExt, WidgetExt}};



pub struct Header;

impl Header {
    pub fn create<P: GroupExt>(parent: &mut P) -> Self {
        let mut row = Flex::default()
            .row()
            .with_size(parent.width(), parent.height() / 7);
        row.end();
        parent.add(&row);

        let mut deimos_icon = SvgImage::from_data(include_str!("../../assets/mars-deimos.svg"))
            .unwrap()
            .copy_sized(row.width(), row.height());
        deimos_icon.normalize();
        let mut rgb = deimos_icon.to_rgb_data();
        for buf in rgb.chunks_mut(4) {
            if buf[3] != 0 {
                buf[0] = 0x96;
                buf[1] = 0x46;
                buf[2] = 0x32;
            }
        }

        let rgb_image = unsafe { RgbImage::from_data(&rgb, deimos_icon.data_w(), deimos_icon.data_h(), fltk::enums::ColorDepth::Rgba8) }.unwrap();

        let mut icon_frame = Frame::default()
            .with_size(row.height(), row.height());
        icon_frame.set_image_scaled(Some(rgb_image));


        row.add(&icon_frame);

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_color(Color::from_rgb(0xff, 0xf4, 0xea));
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);

        row.add(&title_frame);

        Self
    }
}

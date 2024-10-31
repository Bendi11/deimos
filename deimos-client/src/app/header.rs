use fltk::{enums::{Color, Font}, frame::Frame, group::Flex, image::SvgImage, prelude::{GroupExt, WidgetExt}};



pub struct Header;

impl Header {
    pub fn create<P: GroupExt>(parent: &mut P) -> Self {
        let deimos_icon = SvgImage::from_data(include_str!("../../assets/mars-deimos.svg")).unwrap();

        let mut row = Flex::default()
            .row()
            .with_size(parent.width(), parent.height() / 5);
        row.end();
        parent.add(&row);

        let mut icon_frame = Frame::default()
            .with_size(row.height(), row.height());
        icon_frame.set_color(Color::XtermRed);
        icon_frame.set_image_scaled(Some(deimos_icon));

        row.add(&icon_frame);

        let mut title_frame = Frame::default()
            .with_label("Deimos");
        title_frame.set_label_font(Font::CourierBold);
        title_frame.set_label_size(42);

        row.add(&title_frame);

        Self
    }
}

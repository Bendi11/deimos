

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("./assets/icon.ico");
        if let Err(e) = res.compile() {
            panic!("Failed to compile windows resources: {}", e);
        }
    }
}

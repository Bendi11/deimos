const PROTO_DIR: &str = "./proto";

fn main() {
    let proto_files = std::fs::read_dir(PROTO_DIR)
        .expect("Failed to read protobuf directory")
        .filter_map(
            |res| res
                .ok()
                .and_then(|res| res
                    .file_type()
                    .expect("Failed to get filetype for protobuf directory entry")
                    .is_file()
                    .then_some(res.path())
                )
        )
        .collect::<Vec<_>>();

    if let Err(e) = tonic_build::configure()
        .emit_rerun_if_changed(true)
        .server_mod_attribute("deimos", "#[cfg(feature=\"server\")]")
        .client_mod_attribute("deimos", "#[cfg(feature=\"channel\")]")
        .compile_protos(
            &proto_files,
            &[PROTO_DIR]
        ) {
        panic!("Failed to compile protobuf files: {e}");
    }
}

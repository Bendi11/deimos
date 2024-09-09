
fn main() {
    println!("cargo::rerun-if-changed=proto/deimos.proto");

    tonic_build::configure()
        .compile(&["./proto/deimos.proto"], &["./proto"])
        .unwrap()
}

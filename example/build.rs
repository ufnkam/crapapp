fn main() {
    println!("cargo:rerun-if-changed=build_assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("build_assets/icon.ico")
            .compile()
            .expect("failed to embed Windows application icon");
    }
}

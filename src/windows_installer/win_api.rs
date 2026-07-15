#[cfg(feature = "windows")]
pub fn set_icon(icon_path: &str) {
    winresource::WindowsResource::new()
        .set_icon(icon_path)
        .compile()
        .expect("failed to embed Windows application icon");
}

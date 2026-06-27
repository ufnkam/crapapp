use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(
        Path::new(&out_dir).join("payload.rs"),
        "static PAYLOAD: &[PayloadEntry] = &[\n    PayloadEntry { destination: \"$INSTALLPATH/example.exe\", executable: true, bytes: include_bytes!(\"/Users/kamilufnal/workspace/cargo-crapapp/example/target/x86_64-pc-windows-gnu/release/example.exe\") },\n    PayloadEntry { destination: \"$INSTALLPATH/manifests/Cargo.toml\", executable: false, bytes: include_bytes!(\"/Users/kamilufnal/workspace/cargo-crapapp/example/Cargo.toml\") },\n];\n",
    )
    .unwrap();
}

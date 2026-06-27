use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(
        Path::new(&out_dir).join("payload.rs"),
        __CRAPAPP_PAYLOAD_SOURCE__,
    )
    .unwrap();
}

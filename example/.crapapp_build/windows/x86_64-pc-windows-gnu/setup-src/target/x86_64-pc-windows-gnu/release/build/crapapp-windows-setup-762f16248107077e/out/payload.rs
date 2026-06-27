static PAYLOAD: &[PayloadEntry] = &[
    PayloadEntry { destination: "$INSTALLPATH/example.exe", executable: true, bytes: include_bytes!("/Users/kamilufnal/workspace/cargo-crapapp/example/target/x86_64-pc-windows-gnu/release/example.exe") },
    PayloadEntry { destination: "$INSTALLPATH/manifests/Cargo.toml", executable: false, bytes: include_bytes!("/Users/kamilufnal/workspace/cargo-crapapp/example/Cargo.toml") },
];

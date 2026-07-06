#![doc = include_str!("../docs.md")]

/// Command-line entrypoint used by the `cargo-crapapp` binary.
pub mod cli;
/// Build manifest, Cargo metadata, payload, and bundling services.
pub mod services;
/// Runtime Windows installer and uninstaller code embedded into generated setup projects.
pub mod windows_installer;

pub use cli::run_cli;

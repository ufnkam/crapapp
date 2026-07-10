#![doc = include_str!("../docs.md")]

/// Command-line entrypoint used by the `cargo-crapapp` binary.
mod cli;
mod build_config_manifest;
mod build_manifest;
mod build_variable;
mod builder;
mod cargo_package;
mod icons;
mod manifest_file;
mod payload_file;
mod platform_manifest;
mod platform_manifests;
mod target_manifest;
mod windows_bundler;
#[cfg(feature = "windows")]
mod windows_installer;

/// Windows build-script helpers for application binaries.
#[cfg(feature = "windows")]
pub use crate::windows_installer::win_api;

pub use cli::run_cli;

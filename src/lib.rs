#![doc = include_str!("../docs.md")]

mod build_config_manifest;
mod build_manifest;
mod build_variable;
mod builder;
mod bundlers;
mod cargo_package;
/// Command-line entrypoint used by the `cargo-crapapp` binary.
mod cli;
mod icons;
mod linux_installer;
mod macos_installer;
mod manifest_file;
mod package_metadata;
mod payload_file;
mod platform_manifest;
mod platform_manifests;
mod target_manifest;
pub mod windows_installer;

/// Windows build-script helpers for application binaries.
#[cfg(feature = "windows")]
pub use crate::windows_installer::win_api;

pub use cli::run_cli;

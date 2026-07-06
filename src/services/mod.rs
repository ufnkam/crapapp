//! Build-manifest and bundling services used by `cargo crapapp`.
//!
//! This module reads `CRAP.toml`, combines it with Cargo metadata, creates the
//! platform build manifest, and dispatches platform bundlers.

mod build_config_manifest;
mod build_manifest;
mod build_variable;
mod builder;
mod cargo_package;
mod icons;
mod manifest_file;
mod payload_file;
mod platform_manifest;
mod target_manifest;
mod windows_bundler;

pub use build_manifest::{BuildManifest, BuildManifestFormatter};
pub use builder::Builder;
pub use manifest_file::{CrapManifest, MANIFEST_PATH};

//! Windows MSI authoring and bundling.

mod config;
mod msi;
pub mod windows_msi_bundler;

pub use config::{
    AssociatedFile, AssociatedFileKind, DisplayIcon, Eula, InstallerConfig, PayloadEntry, Shortcut,
};

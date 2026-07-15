//! Runtime Windows installer and uninstaller library.
//!
//! Generated Windows setup projects embed this module through `libcrapapp`.
//! The `windows-cli` feature enables command-line setup entrypoints, and the
//! `windows-gui` feature enables graphical setup entrypoints.

#[cfg(feature = "windows-cli")]
pub mod cli;
#[cfg_attr(
    not(any(feature = "windows-cli", feature = "windows-gui")),
    allow(dead_code)
)]
mod config;
#[cfg(feature = "windows-gui")]
pub mod gui;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
mod install;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub mod installer;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
mod registry;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
mod shortcuts;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
mod uninstall;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub mod uninstaller;
#[cfg(feature = "windows")]
pub mod win_api;

#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub use config::{ADD_TO_PATH_VARIABLE, UNINSTALLER_EXE};
pub use config::{
    AssociatedFile, AssociatedFileKind, DisplayIcon, Eula, InstallerConfig, PayloadEntry, Shortcut,
};
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub use install::{
    ExistingInstall, InstallPlan, InstallReport, add_to_path_requested, create_associated_files,
    install_plan, prune_install_root, resolve_variables, validate_variables, write_eula_reports,
};
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub use shortcuts::{create_start_menu_shortcuts, remove_start_menu_shortcuts};
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub use uninstall::{
    remove_associated_files, remove_created_directories, remove_user_path_entries,
    resolve_install_path,
};

//! Runtime Windows installer and uninstaller library.
//!
//! Generated Windows setup projects embed this module through `libcrapapp`.
//! The `windows-cli` feature enables command-line setup entrypoints, and the
//! `windows-gui` feature enables graphical setup entrypoints.

#[cfg(feature = "windows-cli")]
pub mod cli;
mod config;
#[cfg(feature = "windows-gui")]
pub mod gui;
#[cfg_attr(not(feature = "windows-cli"), allow(dead_code))]
mod install;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub mod installer;
#[cfg_attr(not(feature = "windows-cli"), allow(dead_code))]
mod registry;
mod shortcuts;
mod uninstall;
#[cfg(any(feature = "windows-cli", feature = "windows-gui"))]
pub mod uninstaller;
pub mod win_api;

pub use config::{
    ADD_TO_PATH_VARIABLE, AssociatedFile, AssociatedFileKind, Eula, InstallerConfig, PayloadEntry,
    Shortcut, UNINSTALLER_EXE,
};
pub use install::{
    ExistingInstall, InstallPlan, InstallReport, add_to_path_requested, create_associated_files,
    install_plan, prune_install_root, resolve_variables, validate_variables, write_eula_reports,
};
pub use shortcuts::{create_start_menu_shortcuts, remove_start_menu_shortcuts};
pub use uninstall::{
    remove_associated_files, remove_created_directories, remove_user_path_entries,
    resolve_install_path,
};

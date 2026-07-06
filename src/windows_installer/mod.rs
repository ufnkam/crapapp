//! Runtime Windows installer and uninstaller library.
//!
//! Generated Windows setup projects embed this module through `libcrapapp`.
//! The `cli` feature enables command-line setup entrypoints, and the `gui`
//! feature enables graphical setup entrypoints.

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg_attr(not(feature = "cli"), allow(dead_code))]
mod install;
#[cfg(any(feature = "cli", feature = "gui"))]
pub mod installer;
#[cfg_attr(not(feature = "cli"), allow(dead_code))]
mod registry;
mod shortcuts;
mod uninstall;
#[cfg(any(feature = "cli", feature = "gui"))]
pub mod uninstaller;

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

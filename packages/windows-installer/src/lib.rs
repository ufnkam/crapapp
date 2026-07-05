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
mod uninstall;
#[cfg(any(feature = "cli", feature = "gui"))]
pub mod uninstaller;

pub use config::{
    ADD_TO_PATH_VARIABLE, AssociatedFile, AssociatedFileKind, Eula, InstallerConfig, PayloadEntry,
    UNINSTALLER_EXE,
};
pub use install::{
    ExistingInstall, InstallPlan, InstallReport, add_to_path_requested, create_associated_files,
    install_plan, prune_install_root, resolve_variables, validate_variables,
};
pub use uninstall::{
    remove_associated_files, remove_created_directories, remove_user_path_entries,
    resolve_install_path,
};

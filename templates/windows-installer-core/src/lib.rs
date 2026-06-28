pub mod cli;
mod config;
mod install;
mod registry;
mod uninstall;

pub use config::{ADD_TO_PATH_VARIABLE, InstallerConfig, PayloadEntry, UNINSTALLER_EXE};
pub use install::{
    ExistingInstall, InstallPlan, InstallReport, add_to_path_requested, install, install_plan,
    prune_install_root, resolve_variables, validate_variables,
};
pub use uninstall::{UninstallOptions, UninstallReport, uninstall};

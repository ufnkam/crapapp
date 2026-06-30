pub mod cli;
mod config;
mod install;
mod registry;
mod uninstall;

pub use config::{
    ADD_TO_PATH_VARIABLE, IconEntry, InstallerConfig, PayloadEntry, UNINSTALLER_EXE,
    installer_config,
};
pub use install::{
    ExistingInstall, InstallPlan, InstallReport, add_to_path_requested, install_plan,
    prune_install_root, resolve_variables, validate_variables,
};
pub use uninstall::{remove_created_directories, remove_user_path_entries, resolve_install_path};

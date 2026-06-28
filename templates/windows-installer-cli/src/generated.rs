#![allow(unused_macros)]

use crapapp_windows_installer_core::{InstallerConfig, PayloadEntry};

macro_rules! crapapp_template_app_name {
    () => {
        "template-app"
    };
}

macro_rules! crapapp_template_app_version {
    () => {
        "0.0.0"
    };
}

macro_rules! crapapp_template_app_publisher {
    () => {
        None
    };
}

macro_rules! crapapp_template_app_display_icon {
    () => {
        None
    };
}

macro_rules! crapapp_template_uninstaller_bytes {
    () => {
        &[]
    };
}

include!(concat!(env!("OUT_DIR"), "/payload.rs"));

const REQUIRED_VARIABLES: &[&str] = &[
    // crapapp_template_required_variables!()
];
const UNINSTALL_ENTRIES: &[&str] = &[
    // crapapp_template_uninstall_entries!()
];
const PATH_ENTRIES: &[&str] = &[
    // crapapp_template_path_entries!()
];

pub const CONFIG: InstallerConfig = InstallerConfig {
    app_name: crapapp_template_app_name!(),
    app_version: crapapp_template_app_version!(),
    publisher: crapapp_template_app_publisher!(),
    app_display_icon: crapapp_template_app_display_icon!(),
    required_variables: REQUIRED_VARIABLES,
    path_entries: PATH_ENTRIES,
    uninstall_entries: UNINSTALL_ENTRIES,
    uninstaller_bytes: crapapp_template_uninstaller_bytes!(),
    payload: PAYLOAD,
};

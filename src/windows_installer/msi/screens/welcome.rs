use crate::windows_installer::InstallerConfig;

use super::{DialogSpec, display_name};

pub(super) fn dialog(config: &InstallerConfig) -> DialogSpec {
    DialogSpec {
        id: "WelcomeDlg".to_owned(),
        first_control: "Next".to_owned(),
        default_control: "Next".to_owned(),
        cancel_control: "Cancel".to_owned(),
        title: format!("Welcome to the {} Setup Wizard", display_name(config)),
        modeless: false,
    }
}

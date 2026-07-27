use crate::windows_installer::InstallerConfig;

use super::{DialogSpec, display_name};

pub(super) fn dialog(config: &InstallerConfig) -> DialogSpec {
    DialogSpec {
        id: "VerifyReadyDlg".to_owned(),
        first_control: "Install".to_owned(),
        default_control: "Install".to_owned(),
        cancel_control: "Cancel".to_owned(),
        title: format!("Ready to Install {}", display_name(config)),
        modeless: false,
    }
}

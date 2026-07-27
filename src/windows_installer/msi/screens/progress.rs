use crate::windows_installer::InstallerConfig;

use super::{DialogSpec, display_name};

pub(super) fn dialog(config: &InstallerConfig) -> DialogSpec {
    DialogSpec {
        id: "ProgressDlg".to_owned(),
        first_control: "Progress".to_owned(),
        default_control: "Cancel".to_owned(),
        cancel_control: "Cancel".to_owned(),
        title: format!("Installing {}", display_name(config)),
        modeless: true,
    }
}

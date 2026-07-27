use crate::windows_installer::InstallerConfig;

use super::{DialogSpec, display_name};

pub(super) fn dialog(config: &InstallerConfig) -> DialogSpec {
    DialogSpec {
        id: "ExitDlg".to_owned(),
        first_control: "Finish".to_owned(),
        default_control: "Finish".to_owned(),
        cancel_control: "Finish".to_owned(),
        title: format!("{} Setup Complete", display_name(config)),
        modeless: false,
    }
}

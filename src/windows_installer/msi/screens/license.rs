use crate::windows_installer::Eula;

use super::DialogSpec;

pub(super) fn dialog((index, eula): (usize, &Eula)) -> DialogSpec {
    DialogSpec {
        id: format!("LicenseDlg{}", index + 1),
        first_control: "LicenseText".to_owned(),
        default_control: "Next".to_owned(),
        cancel_control: "Cancel".to_owned(),
        title: format!("License Agreement - {}", eula.name),
        modeless: false,
    }
}

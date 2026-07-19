use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum WindowsInstallerKind {
    Cli,
    Gui,
    Msi,
}

impl WindowsInstallerKind {
    pub fn cargo_feature(self) -> &'static str {
        match self {
            WindowsInstallerKind::Cli => "cli",
            WindowsInstallerKind::Gui => "gui",
            WindowsInstallerKind::Msi => "msi",
        }
    }

    pub fn installer_binary_name(self) -> &'static str {
        match self {
            WindowsInstallerKind::Cli | WindowsInstallerKind::Gui => "setup.exe",
            WindowsInstallerKind::Msi => "setup.msi",
        }
    }
}

use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum WindowsBundlerKind {
    Cli,
    Gui,
    Msi,
}

impl WindowsBundlerKind {
    pub fn cargo_feature(self) -> &'static str {
        match self {
            WindowsBundlerKind::Cli => "cli",
            WindowsBundlerKind::Gui => "gui",
            WindowsBundlerKind::Msi => "msi",
        }
    }

    pub fn installer_binary_name(self) -> &'static str {
        match self {
            WindowsBundlerKind::Cli | WindowsBundlerKind::Gui => "setup.exe",
            WindowsBundlerKind::Msi => "setup.msi",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum LinuxBundlerKind {
    Deb,
    Rpm,
    Aur,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MacosBundlerKind {
    App,
    Pkg,
    Dmg,
}

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash, Display, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum WindowsBundlerKind {
    Msi,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash, Display, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum LinuxBundlerKind {
    Deb,
    Rpm,
    Aur,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash, Display, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MacosBundlerKind {
    App,
    Pkg,
    Dmg,
}

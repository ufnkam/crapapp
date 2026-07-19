use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MacosInstallerKind {
    App,
    Pkg,
}

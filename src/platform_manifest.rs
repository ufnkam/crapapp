use std::fmt::Debug;

use crate::{
    platform_manifests::{LinuxPlatformManifest, MacosPlatformManifest, WindowsPlatformManifest},
    target_manifest::TargetManifest,
};
use serde::Serialize;

pub trait PlatformManifest: Debug + Serialize {
    fn platform(&self) -> &str;
    fn targets(&self) -> Vec<&str>;
    fn write_text(&self, output: &mut String);
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PlatformBuildManifest {
    Windows(WindowsPlatformManifest<TargetManifest>),
    Macos(MacosPlatformManifest<TargetManifest>),
    Linux(LinuxPlatformManifest<TargetManifest>),
}

impl PlatformManifest for PlatformBuildManifest {
    fn platform(&self) -> &str {
        match self {
            Self::Windows(platform) => platform.platform(),
            Self::Macos(platform) => platform.platform(),
            Self::Linux(platform) => platform.platform(),
        }
    }

    fn targets(&self) -> Vec<&str> {
        match self {
            Self::Windows(platform) => platform.targets(),
            Self::Macos(platform) => platform.targets(),
            Self::Linux(platform) => platform.targets(),
        }
    }

    fn write_text(&self, output: &mut String) {
        match self {
            Self::Windows(platform) => platform.write_text(output),
            Self::Macos(platform) => platform.write_text(output),
            Self::Linux(platform) => platform.write_text(output),
        }
    }
}

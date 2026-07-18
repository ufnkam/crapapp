use std::fmt::Debug;

use crate::{
    platform_manifests::{MacosPlatformManifest, WindowsPlatformManifest},
    target_manifest::TargetManifest,
};
use serde::{Deserialize, Serialize};

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
    Linux(BasicPlatformManifest),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct BasicPlatformManifest {
    pub platform: String,
    pub targets: Vec<TargetManifest>,
}

impl PlatformManifest for BasicPlatformManifest {
    fn platform(&self) -> &str {
        &self.platform
    }

    fn targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.target.as_str())
            .collect()
    }

    fn write_text(&self, output: &mut String) {
        output.push_str(&format!("  {}\n", self.platform));

        for target in &self.targets {
            output.push_str(&format!("    {}\n", target.target));

            for file in &target.files {
                let marker = if file.executable { "x" } else { "-" };
                output.push_str(&format!(
                    "      [{}] {} -> {}\n",
                    marker, file.source, file.destination
                ));
            }
        }
    }
}

impl BasicPlatformManifest {
    pub fn new(platform: &str, targets: Vec<TargetManifest>) -> Self {
        Self {
            platform: platform.to_owned(),
            targets,
        }
    }
}

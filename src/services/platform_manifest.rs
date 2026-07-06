use std::fmt::Debug;

use crate::services::build_variable::{BuildVariable, platform_variables};
use crate::services::manifest_file::{AssociatedFile, EulaFile, WindowsInstaller};
use crate::services::payload_file::PayloadFile;
use crate::services::target_manifest::TargetManifest;
use serde::Serialize;
use serde_json::Value;

pub trait PlatformManifest: Debug + Serialize {
    fn platform(&self) -> &str;
    fn targets(&self) -> Vec<&str>;
    fn to_json(&self) -> Result<Value, serde_json::Error>;
    fn write_text(&self, output: &mut String);
    fn as_windows(&self) -> Option<&WindowsPlatformManifest> {
        None
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PlatformBuildManifest {
    Windows(WindowsPlatformManifest),
    Macos(BasicPlatformManifest),
    Linux(BasicPlatformManifest),
}

impl PlatformManifest for PlatformBuildManifest {
    fn platform(&self) -> &str {
        match self {
            Self::Windows(platform) => platform.platform(),
            Self::Macos(platform) | Self::Linux(platform) => platform.platform(),
        }
    }

    fn targets(&self) -> Vec<&str> {
        match self {
            Self::Windows(platform) => platform.targets(),
            Self::Macos(platform) | Self::Linux(platform) => platform.targets(),
        }
    }

    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    fn write_text(&self, output: &mut String) {
        match self {
            Self::Windows(platform) => platform.write_text(output),
            Self::Macos(platform) | Self::Linux(platform) => platform.write_text(output),
        }
    }

    fn as_windows(&self) -> Option<&WindowsPlatformManifest> {
        match self {
            Self::Windows(platform) => Some(platform),
            Self::Macos(_) | Self::Linux(_) => None,
        }
    }
}

#[derive(Debug, Serialize)]
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

    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
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

#[derive(Debug, Serialize)]
pub struct WindowsPlatformManifest {
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<WindowsInstaller>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_icon_source: Option<String>,
    pub associated_files: Vec<AssociatedFile>,
    pub eulas: Vec<EulaFile>,
    pub variables: Vec<BuildVariable>,
    pub targets: Vec<TargetManifest>,
}

impl PlatformManifest for WindowsPlatformManifest {
    fn platform(&self) -> &str {
        &self.platform
    }

    fn targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.target.as_str())
            .collect()
    }

    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    fn write_text(&self, output: &mut String) {
        output.push_str(&format!("  {}\n", self.platform));

        if !self.variables.is_empty() {
            let variables = self
                .variables
                .iter()
                .map(|variable| variable.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            output.push_str(&format!("    variables: {variables}\n"));
        }

        if let Some(display_icon) = &self.display_icon {
            output.push_str(&format!("    display icon: {display_icon}\n"));
        }

        if !self.associated_files.is_empty() {
            output.push_str("    associated files:\n");

            for file in &self.associated_files {
                output.push_str(&format!("      {:?}: {}\n", file.kind, file.path));
            }
        }

        if !self.eulas.is_empty() {
            output.push_str("    eulas:\n");

            for eula in &self.eulas {
                let required = if eula.required() {
                    "required"
                } else {
                    "optional"
                };
                output.push_str(&format!("      {} ({required})\n", eula.path()));
            }
        }

        for target in &self.targets {
            output.push_str(&format!("    {}\n", target.target));

            for file in &target.files {
                let marker = if file.executable { "x" } else { "-" };
                output.push_str(&format!(
                    "      [{}] {} -> {}\n",
                    marker, file.source, file.destination
                ));
            }

            if !target.shortcuts.is_empty() {
                output.push_str("      shortcuts:\n");

                for shortcut in &target.shortcuts {
                    if let Some(directory) = &shortcut.directory {
                        output.push_str(&format!(
                            "        {} -> {} ({directory})\n",
                            shortcut.name, shortcut.target
                        ));
                    } else {
                        output.push_str(&format!(
                            "        {} -> {}\n",
                            shortcut.name, shortcut.target
                        ));
                    }
                }
            }
        }
    }

    fn as_windows(&self) -> Option<&WindowsPlatformManifest> {
        Some(self)
    }
}

impl WindowsPlatformManifest {
    pub fn new(
        platform: &str,
        installer: Option<WindowsInstaller>,
        display_icon: Option<&str>,
        display_icon_source: Option<&str>,
        associated_files: &[AssociatedFile],
        eulas: &[EulaFile],
        variable_sources: &[&str],
        files: &[PayloadFile],
        targets: Vec<TargetManifest>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            platform: platform.to_owned(),
            installer,
            display_icon: display_icon.map(str::to_owned),
            display_icon_source: display_icon_source.map(str::to_owned),
            associated_files: associated_files.to_vec(),
            eulas: eulas.to_vec(),
            variables: platform_variables(variable_sources, files)?,
            targets,
        })
    }
}

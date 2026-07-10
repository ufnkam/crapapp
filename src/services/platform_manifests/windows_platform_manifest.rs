use serde::Serialize;

use crate::services::{
    build_variable::{BuildVariable, get_platform_variables},
    manifest_file::{AssociatedFile, EulaFile, WindowsInstaller},
    payload_file::PayloadFile,
    platform_manifest::PlatformManifest,
    target_manifest::TargetManifest,
};

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
                let eula_report = if file.eula_report {
                    " (EULA report)"
                } else {
                    ""
                };
                output.push_str(&format!(
                    "      {:?}: {}{}\n",
                    file.kind, file.path, eula_report
                ));
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
            variables: get_platform_variables(variable_sources, files)?,
            targets,
        })
    }
}

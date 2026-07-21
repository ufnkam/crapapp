use serde::{Deserialize, Serialize};

use crate::bundlers::WindowsBundlerKind;
use crate::{
    build_variable::{BuildVariable, get_platform_variables},
    manifest_file::{AssociatedFile, EulaFile, FileMapping, ShortcutMapping, WindowsTarget},
    payload_file::PayloadFile,
    platform_manifest::PlatformManifest,
    target_manifest::TargetManifest,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    deserialize = "Target: Deserialize<'de>",
    serialize = "Target: Serialize"
))]
pub struct WindowsPlatformManifest<Target = WindowsTarget> {
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(
        default,
        deserialize_with = "crate::manifest_file::deserialize_windows_bundles"
    )]
    pub bundle: Vec<WindowsBundlerKind>,
    pub install_path: Option<String>,
    pub bin_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileMapping>,
    #[serde(default)]
    pub associated_files: Vec<AssociatedFile>,
    #[serde(default)]
    pub eulas: Vec<EulaFile>,
    #[serde(default)]
    pub shortcuts: Vec<ShortcutMapping>,
    pub display_icon: Option<String>,
    #[serde(skip, default)]
    pub display_icon_source: Option<String>,
    #[serde(skip, default)]
    pub variables: Vec<BuildVariable>,
}

impl<Target> WindowsPlatformManifest<Target> {
    pub fn bundles(&self) -> Vec<WindowsBundlerKind> {
        let bundles = if self.bundle.is_empty() {
            vec![WindowsBundlerKind::Cli]
        } else {
            self.bundle.clone()
        };
        let mut unique_bundles = Vec::with_capacity(bundles.len());

        for bundle in bundles {
            if !unique_bundles.contains(&bundle) {
                unique_bundles.push(bundle);
            }
        }

        unique_bundles
    }
}

impl WindowsPlatformManifest {
    pub fn bin_dir(&self) -> &str {
        self.bin_dir.as_deref().unwrap_or("")
    }

    pub fn install_path(&self) -> Option<&str> {
        self.install_path.as_deref()
    }

    pub fn variable_sources(&self) -> Vec<&str> {
        self.install_path
            .iter()
            .map(String::as_str)
            .chain(self.associated_files.iter().map(|file| file.path.as_str()))
            .collect()
    }

    pub fn display_icon(&self) -> Option<&str> {
        self.display_icon.as_deref()
    }

    pub fn build(
        &self,
        files: &[PayloadFile],
        targets: Vec<TargetManifest>,
        variable_sources: &[&str],
        display_icon: Option<&str>,
        display_icon_source: Option<&str>,
    ) -> anyhow::Result<WindowsPlatformManifest<TargetManifest>> {
        Ok(WindowsPlatformManifest {
            platform: "windows".to_owned(),
            targets,
            bundle: self.bundles(),
            install_path: self.install_path.clone(),
            bin_dir: self.bin_dir.clone(),
            files: Vec::new(),
            associated_files: self.associated_files.clone(),
            eulas: self.eulas.clone(),
            shortcuts: self.shortcuts.clone(),
            display_icon: display_icon.map(str::to_owned),
            display_icon_source: display_icon_source.map(str::to_owned),
            variables: get_platform_variables(variable_sources, files)?,
        })
    }
}

impl PlatformManifest for WindowsPlatformManifest<TargetManifest> {
    fn platform(&self) -> &str {
        if self.platform.is_empty() {
            "windows"
        } else {
            &self.platform
        }
    }

    fn targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.target.as_str())
            .collect()
    }

    fn write_text(&self, output: &mut String) {
        output.push_str(&format!("  {}\n", self.platform()));

        if !self.bundle.is_empty() {
            let bundles = self
                .bundle
                .iter()
                .map(|bundle| bundle.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            output.push_str(&format!("    bundle: {bundles}\n"));
        }

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
                    output.push_str(&format!("        {} -> {}", shortcut.name, shortcut.target));

                    if let Some(directory) = &shortcut.directory {
                        output.push_str(&format!(" ({directory})"));
                    }

                    if let Some(icon) = &shortcut.icon {
                        output.push_str(&format!(" [icon: {icon}]"));
                    }

                    output.push('\n');
                }
            }
        }
    }
}

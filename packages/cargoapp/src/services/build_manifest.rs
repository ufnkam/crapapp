use anyhow::{Context, Result};
use serde::Serialize;

use crate::services::build_config_manifest::BuildConfigManifest;
use crate::services::cargo_package::CargoPackage;
use crate::services::icons::validate_display_icon;
use crate::services::manifest_file::{CrapManifest, PlatformConfig, PlatformManifest as _};
use crate::services::payload_file::{join_payload_path, payload_files, resolve_destination};
use crate::services::platform_manifest::PlatformManifest;
use crate::services::target_manifest::TargetManifest;

#[derive(Debug, Serialize)]
pub struct BuildManifest {
    pub app_name: String,
    pub version: String,
    pub build: BuildConfigManifest,
    pub platforms: Vec<PlatformManifest>,
}

impl BuildManifest {
    pub fn from_crap_manifest(manifest: &CrapManifest) -> Result<Self> {
        let cargo_package = CargoPackage::load()?;
        let mut platforms = Vec::new();

        for platform in manifest.platforms() {
            let files = payload_files(platform.files(), platform.install_path())?;
            validate_display_icon(platform.display_icon())?;
            let mut targets = Vec::new();
            let variable_sources = platform.variable_sources();
            let installer = match &platform {
                PlatformConfig::Windows(windows) => Some(windows.installer),
                PlatformConfig::Macos(_) | PlatformConfig::Linux(_) => None,
            };
            let display_icon =
                display_icon_destination(&platform, &cargo_package.name, &cargo_package.binaries);

            for target in platform.targets() {
                targets.push(TargetManifest::new(
                    target,
                    &cargo_package.binaries,
                    platform.install_path(),
                    platform.bin_dir(),
                    &files,
                ));
            }

            platforms.push(PlatformManifest::new(
                platform.name(),
                installer,
                display_icon.as_deref(),
                &variable_sources,
                &files,
                targets,
            )?);
        }

        Ok(Self {
            app_name: cargo_package.name,
            version: cargo_package.version,
            build: BuildConfigManifest::from_crap_manifest(manifest),
            platforms,
        })
    }

    pub fn display(&self, formatter: BuildManifestFormatter) -> Result<String> {
        match formatter {
            BuildManifestFormatter::Text => Ok(self.display_text()),
            BuildManifestFormatter::Json => {
                serde_json::to_string_pretty(self).context("failed to render build manifest")
            }
        }
    }

    fn display_text(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("app: {}\n", self.app_name));
        output.push_str(&format!("version: {}\n", self.version));

        if let Some(publisher) = &self.build.publisher {
            output.push_str(&format!("publisher: {publisher}\n"));
        }

        if !self.build.packages.is_empty() {
            output.push_str(&format!("packages: {}\n", self.build.packages.join(", ")));
        }

        if !self.build.features.is_empty() {
            output.push_str(&format!("features: {}\n", self.build.features.join(", ")));
        }

        output.push_str("platforms:\n");

        for platform in &self.platforms {
            output.push_str(&format!("  {}\n", platform.platform));

            if !platform.variables.is_empty() {
                let variables = platform
                    .variables
                    .iter()
                    .map(|variable| variable.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                output.push_str(&format!("    variables: {variables}\n"));
            }

            if let Some(display_icon) = &platform.display_icon {
                output.push_str(&format!("    display icon: {display_icon}\n"));
            }

            for target in &platform.targets {
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

        output
    }
}

fn display_icon_destination(
    platform: &impl crate::services::manifest_file::PlatformManifest,
    package_name: &str,
    binary_names: &[String],
) -> Option<String> {
    platform.display_icon()?;
    let binary_name = binary_names
        .iter()
        .find(|binary| binary.as_str() == package_name)
        .or_else(|| binary_names.first())?;
    let binary_file_name = if platform.name() == "windows" {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_owned()
    };

    Some(resolve_destination(
        platform.install_path(),
        &join_payload_path(platform.bin_dir(), &binary_file_name),
    ))
}

#[derive(Clone, Copy, Debug)]
pub enum BuildManifestFormatter {
    Text,
    Json,
}

use anyhow::{Context, Result};
use serde::Serialize;

use crate::services::build_config_manifest::BuildConfigManifest;
use crate::services::build_variable::{BuildVariable, platform_variables};
use crate::services::cargo_package::CargoPackage;
use crate::services::icons::{IconMapping, validate_icons};
use crate::services::manifest_file::{CrapManifest, PlatformManifest as _};
use crate::services::payload_file::{PayloadFile, payload_files};
use crate::services::target_manifest::{TargetManifest, validate_target_supported};

#[derive(Debug, Serialize)]
pub struct BuildManifest {
    pub app_name: String,
    pub version: String,
    pub build: BuildConfigManifest,
    pub platforms: Vec<PlatformManifest>,
}

#[derive(Debug, Serialize)]
pub struct PlatformManifest {
    pub platform: String,
    pub variables: Vec<BuildVariable>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<IconMapping>,
    pub targets: Vec<TargetManifest>,
}

impl BuildManifest {
    pub fn from_crap_manifest(manifest: &CrapManifest) -> Result<Self> {
        let cargo_package = CargoPackage::load()?;
        let mut platforms = Vec::new();

        for platform in manifest.platforms() {
            let files = payload_files(platform.files(), platform.install_path())?;
            validate_icons(platform.name(), platform.icons(), &cargo_package.binaries)?;
            let mut targets = Vec::new();
            let variable_sources = platform.variable_sources();

            for target in platform.targets() {
                validate_target_supported(target)?;
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
                &variable_sources,
                &files,
                platform.icons().to_vec(),
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
                    .map(|variable| variable.name())
                    .collect::<Vec<_>>()
                    .join(", ");

                output.push_str(&format!("    variables: {variables}\n"));
            }

            for icon in &platform.icons {
                output.push_str(&format!("    icon: {} -> {}\n", icon.binary, icon.source));
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

#[derive(Clone, Copy, Debug)]
pub enum BuildManifestFormatter {
    Text,
    Json,
}

impl PlatformManifest {
    fn new(
        platform: &str,
        variable_sources: &[&str],
        files: &[PayloadFile],
        icons: Vec<IconMapping>,
        targets: Vec<TargetManifest>,
    ) -> Result<Self> {
        Ok(Self {
            platform: platform.to_owned(),
            variables: platform_variables(variable_sources, files)?,
            icons,
            targets,
        })
    }
}

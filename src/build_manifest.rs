use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::build_config_manifest::BuildConfigManifest;
use crate::bundlers::shared;
use crate::cargo_package::CargoPackage;
use crate::icons::validate_display_icon;
use crate::manifest_file::{
    CrapManifest, PlatformConfig, PlatformManifest as SourcePlatformManifest,
};
use crate::payload_file::{payload_files, resolve_destination};
use crate::platform_manifest::{BasicPlatformManifest, PlatformBuildManifest, PlatformManifest};
use crate::platform_manifests::MacosPlatformManifest;
use crate::target_manifest::TargetManifest;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct BuildManifest {
    pub app_name: String,
    pub version: String,
    pub build: BuildConfigManifest,
    pub platforms: Vec<PlatformBuildManifest>,
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
            let display_icon_source = platform.display_icon();
            let windows_display_icon =
                display_icon_destination(&platform, &cargo_package.name, &cargo_package.binaries);
            let macos_display_icon = match display_icon_source {
                Some(source) => Some(shared::icon_file_name(source)?.to_owned()),
                None => None,
            };
            let macos_app_binary = match &platform {
                PlatformConfig::Macos(macos) => {
                    let app_binary = macos
                        .app_binary
                        .as_deref()
                        .filter(|app_binary| !app_binary.trim().is_empty());
                    validate_macos_app_binary(app_binary, &cargo_package.binaries)?;
                    app_binary
                }
                PlatformConfig::Windows(_) | PlatformConfig::Linux(_) => None,
            };
            let macos_bundles = match &platform {
                PlatformConfig::Macos(macos) => macos.bundles(),
                PlatformConfig::Windows(_) | PlatformConfig::Linux(_) => Vec::new(),
            };
            let macos_pkg_config = match &platform {
                PlatformConfig::Macos(macos) => macos.pkg.clone(),
                PlatformConfig::Windows(_) | PlatformConfig::Linux(_) => Default::default(),
            };
            let macos_eulas = match &platform {
                PlatformConfig::Macos(macos) => macos.eulas.clone(),
                PlatformConfig::Windows(_) | PlatformConfig::Linux(_) => Vec::new(),
            };
            let shortcuts = match &platform {
                PlatformConfig::Windows(windows) => windows.shortcuts.as_slice(),
                PlatformConfig::Macos(_) | PlatformConfig::Linux(_) => &[],
            };
            validate_shortcut_icons(shortcuts)?;

            for target in platform.targets() {
                targets.push(TargetManifest::new(
                    target,
                    &cargo_package.binaries,
                    platform.install_path(),
                    platform.bin_dir(),
                    &files,
                    shortcuts,
                )?);
            }

            platforms.push(match &platform {
                PlatformConfig::Windows(windows) => PlatformBuildManifest::Windows(windows.build(
                    &files,
                    targets,
                    &variable_sources,
                    windows_display_icon.as_deref(),
                    display_icon_source,
                )?),
                PlatformConfig::Macos(_) => {
                    PlatformBuildManifest::Macos(MacosPlatformManifest::new(
                        targets,
                        macos_display_icon.as_deref(),
                        display_icon_source,
                        macos_app_binary,
                        macos_bundles,
                        macos_pkg_config,
                        macos_eulas,
                    ))
                }
                PlatformConfig::Linux(_) => PlatformBuildManifest::Linux(
                    BasicPlatformManifest::new(platform.name(), targets),
                ),
            });
        }

        Ok(Self {
            app_name: cargo_package.name,
            version: cargo_package.version,
            build: BuildConfigManifest::from_crap_manifest(manifest),
            platforms,
        })
    }
}

fn validate_shortcut_icons(shortcuts: &[crate::manifest_file::ShortcutMapping]) -> Result<()> {
    for shortcut in shortcuts {
        validate_display_icon(shortcut.icon.as_deref())?;
    }

    Ok(())
}

fn validate_macos_app_binary(app_binary: Option<&str>, binary_names: &[String]) -> Result<()> {
    let Some(app_binary) = app_binary else {
        return Ok(());
    };

    if binary_names.iter().any(|binary| binary == app_binary) {
        return Ok(());
    }

    bail!("macOS app_binary references unknown binary {app_binary}");
}

impl BuildManifest {
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

        if let Some(display_name) = &self.build.display_name {
            output.push_str(&format!("display name: {display_name}\n"));
        }

        if !self.build.packages.is_empty() {
            output.push_str(&format!("packages: {}\n", self.build.packages.join(", ")));
        }

        if !self.build.features.is_empty() {
            output.push_str(&format!("features: {}\n", self.build.features.join(", ")));
        }

        output.push_str("platforms:\n");

        for platform in &self.platforms {
            platform.write_text(&mut output);
        }

        output
    }

    pub fn get_platform_config(&self, platform: &str) -> Option<&PlatformBuildManifest> {
        self.platforms
            .iter()
            .find(|_platform| _platform.platform() == platform)
    }
}

fn display_icon_destination(
    platform: &impl crate::manifest_file::PlatformManifest,
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
        &Path::new(platform.bin_dir())
            .join(&binary_file_name)
            .display()
            .to_string(),
    ))
}

#[derive(Clone, Copy, Debug)]
pub enum BuildManifestFormatter {
    Text,
    Json,
}

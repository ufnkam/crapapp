use anyhow::{Context, Result, bail};
use cargo_metadata::{MetadataCommand, TargetKind};
use serde::Serialize;

use crate::services::manifest_file::{CrapManifest, FileMapping, PlatformManifest as _};
use crate::services::toolchain_manifest::{
    BuildVariable, PayloadFile, ToolchainManifest, resolve_destination, variables_from_files,
    variables_from_value,
};

#[derive(Debug, Serialize)]
pub struct BuildManifest {
    pub app_name: String,
    pub version: String,
    pub cargo: CargoBuildManifest,
    pub platforms: Vec<PlatformManifest>,
}

#[derive(Debug, Serialize)]
pub struct CargoBuildManifest {
    pub packages: Vec<String>,
    pub features: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PlatformManifest {
    pub platform: String,
    pub variables: Vec<BuildVariable>,
    pub toolchains: Vec<ToolchainManifest>,
}

impl BuildManifest {
    pub fn from_crap_manifest(manifest: &CrapManifest) -> Result<Self> {
        let cargo_package = CargoPackage::load()?;
        let mut platforms = Vec::new();

        for platform in manifest.platforms() {
            let files = payload_files(platform.files(), platform.install_path());
            let mut toolchains = Vec::new();
            let variable_sources = platform.variable_sources();

            for rust_target in platform.rust_targets() {
                validate_toolchain_supported(rust_target)?;
                toolchains.push(ToolchainManifest::new(
                    rust_target,
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
                toolchains,
            )?);
        }

        Ok(Self {
            app_name: cargo_package.name,
            version: cargo_package.version,
            cargo: CargoBuildManifest::from_crap_manifest(manifest),
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

        if !self.cargo.packages.is_empty() {
            output.push_str(&format!("packages: {}\n", self.cargo.packages.join(", ")));
        }

        if !self.cargo.features.is_empty() {
            output.push_str(&format!("features: {}\n", self.cargo.features.join(", ")));
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

            for toolchain in &platform.toolchains {
                output.push_str(&format!("    {}\n", toolchain.toolchain));

                for file in &toolchain.files {
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

    #[allow(dead_code)]
    pub fn variables_for_platform(&self, platform: &str) -> Option<Vec<&str>> {
        self.platforms
            .iter()
            .find(|platform_manifest| platform_manifest.platform == platform)
            .map(|platform_manifest| {
                platform_manifest
                    .variables
                    .iter()
                    .map(|variable| variable.name())
                    .collect()
            })
    }
}

impl CargoBuildManifest {
    fn from_crap_manifest(manifest: &CrapManifest) -> Self {
        let Some(cargo) = &manifest.cargo else {
            return Self {
                packages: Vec::new(),
                features: Vec::new(),
            };
        };

        Self {
            packages: cargo.packages.clone(),
            features: cargo.features.clone(),
        }
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
        toolchains: Vec<ToolchainManifest>,
    ) -> Result<Self> {
        Ok(Self {
            platform: platform.to_owned(),
            variables: platform_variables(variable_sources, files)?,
            toolchains,
        })
    }
}

fn platform_variables(
    variable_sources: &[&str],
    files: &[PayloadFile],
) -> Result<Vec<BuildVariable>> {
    let mut variables = variable_sources
        .iter()
        .flat_map(|value| variables_from_value(value))
        .map(|name| BuildVariable::try_from(name.as_str()))
        .collect::<Result<Vec<_>>>()?;

    variables.extend(variables_from_files(files)?);
    variables.sort();
    variables.dedup();
    Ok(variables)
}

fn payload_files(files: &[FileMapping], install_path: Option<&str>) -> Vec<PayloadFile> {
    files
        .iter()
        .map(|file| {
            PayloadFile::data(
                file.source.clone(),
                resolve_destination(install_path, &file.destination),
            )
        })
        .collect()
}

fn validate_toolchain_supported(rust_target: &str) -> Result<()> {
    if rust_target.ends_with("pc-windows-msvc") && !cfg!(target_os = "windows") {
        bail!("{rust_target} can only be built on Windows");
    }

    if rust_target.ends_with("apple-darwin") && !cfg!(target_os = "macos") {
        bail!("{rust_target} can only be built on macOS");
    }

    Ok(())
}

struct CargoPackage {
    name: String,
    version: String,
    binaries: Vec<String>,
}

impl CargoPackage {
    fn load() -> Result<Self> {
        let metadata = MetadataCommand::new()
            .no_deps()
            .exec()
            .context("failed to read cargo metadata")?;

        let root_package = metadata
            .root_package()
            .context("failed to find root cargo package")?;

        let binaries = root_package
            .targets
            .iter()
            .filter(|target| target.kind.contains(&TargetKind::Bin))
            .map(|target| target.name.to_string())
            .collect::<Vec<_>>();

        if binaries.is_empty() {
            bail!("cargo package does not define any binary targets");
        }

        Ok(Self {
            name: root_package.name.to_string(),
            version: root_package.version.to_string(),
            binaries,
        })
    }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::services::build_manifest::BuildManifest;
use crate::services::build_variable::BuildVariable;
use crate::services::icons::IconMapping;
use crate::services::payload_file::PayloadFile;
use crate::services::platform_manifest::PlatformManifest;

const SETUP_CARGO_TOML: &str = include_str!("../../templates/windows-installer-cli/Cargo.toml");
const SETUP_BUILD_RS: &str = include_str!("../../templates/assets/build.rs.j2");
const SETUP_RS: &str = include_str!("../../templates/windows-installer-cli/src/setup.rs");
const UNINSTALL_RS: &str = include_str!("../../templates/windows-installer-cli/src/uninstall.rs");
const CORE_CARGO_TOML: &str = include_str!("../../templates/windows-installer-core/Cargo.toml");
const CORE_RS: &str = include_str!("../../templates/windows-installer-core/src/lib.rs");
const CORE_CONFIG_RS: &str = include_str!("../../templates/windows-installer-core/src/config.rs");
const CORE_CLI_RS: &str = include_str!("../../templates/windows-installer-core/src/cli.rs");
const CORE_INSTALL_RS: &str = include_str!("../../templates/windows-installer-core/src/install.rs");
const CORE_REGISTRY_RS: &str =
    include_str!("../../templates/windows-installer-core/src/registry.rs");
const CORE_UNINSTALL_RS: &str =
    include_str!("../../templates/windows-installer-core/src/uninstall.rs");
const INSTALL_ICON: &[u8] = include_bytes!("../../templates/assets/install.ico");
const GENERATED_WORKSPACE_PLACEHOLDER: &str = "# crapapp_template_generated_workspace!()";
const GENERATED_CORE_DEPENDENCY_PLACEHOLDER: &str =
    r#"windows-installer-core = { path = "../windows-installer-core" }"#;
const GENERATED_CORE_DEPENDENCY: &str =
    r#"windows-installer-core = { path = "windows-installer-core" }"#;
const SETUP_CONFIG: &str = "setup-config.json";

pub struct WindowsBundler<'a> {
    build_manifest: &'a BuildManifest,
    build_dir: &'a Path,
}

impl<'a> WindowsBundler<'a> {
    pub fn new(build_manifest: &'a BuildManifest, build_dir: &'a Path) -> Self {
        Self {
            build_manifest,
            build_dir,
        }
    }

    pub fn bundle(&self) -> Result<()> {
        let windows = self.windows_platform()?;

        for target in &windows.targets {
            let output_dir = self.build_dir.join(&windows.platform).join(&target.target);
            let setup_source_dir = output_dir.join("setup-src");
            let setup_output = output_dir.join("setup.exe");

            remove_dir_if_exists(&setup_source_dir)?;
            fs::create_dir_all(&output_dir)
                .with_context(|| format!("failed to create {}", output_dir.display()))?;
            fs::create_dir_all(setup_source_dir.join("src")).with_context(|| {
                format!(
                    "failed to create setup project at {}",
                    setup_source_dir.display()
                )
            })?;

            self.write_setup_project(windows, &target.files, &setup_source_dir)?;
            self.build_uninstaller(&target.target, &setup_source_dir)?;
            self.write_setup_rs_with_uninstaller(windows, &target.target, &setup_source_dir)?;
            self.build_setup(&target.target, &setup_source_dir)?;
            self.copy_setup_output(&target.target, &setup_source_dir, &setup_output)?;
            self.clean_setup_source(&setup_source_dir)?;
        }

        Ok(())
    }

    fn windows_platform(&self) -> Result<&PlatformManifest> {
        self.build_manifest
            .platforms
            .iter()
            .find(|platform| platform.platform == "windows")
            .context("windows platform is not configured")
    }

    fn write_setup_project(
        &self,
        platform: &PlatformManifest,
        files: &[PayloadFile],
        setup_source_dir: &Path,
    ) -> Result<()> {
        if files.is_empty() {
            bail!("windows bundle has no files to package");
        }

        fs::write(setup_source_dir.join("Cargo.toml"), setup_cargo_toml())
            .with_context(|| "failed to write setup Cargo.toml")?;
        fs::write(setup_source_dir.join("build.rs"), SETUP_BUILD_RS)
            .with_context(|| "failed to write setup build.rs")?;
        self.write_core_project(setup_source_dir)?;
        self.write_setup_build_input(platform, files, None, setup_source_dir)?;
        write_setup_assets(setup_source_dir)?;
        fs::write(setup_source_dir.join("src").join("setup.rs"), SETUP_RS)
            .with_context(|| "failed to write setup.rs")?;
        fs::write(
            setup_source_dir.join("src").join("uninstall.rs"),
            UNINSTALL_RS,
        )
        .with_context(|| "failed to write uninstall.rs")?;

        Ok(())
    }

    fn build_uninstaller(&self, target: &str, setup_source_dir: &Path) -> Result<()> {
        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(target)
            .arg("--bin")
            .arg("uninstall")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build uninstall.exe for {target}"))?;

        if !status.success() {
            bail!("uninstall.exe build failed for {target}");
        }

        Ok(())
    }

    fn write_setup_rs_with_uninstaller(
        &self,
        platform: &PlatformManifest,
        target: &str,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let uninstaller = fs::canonicalize(release_exe_path(target, setup_source_dir, "uninstall"))
            .with_context(|| format!("failed to find built uninstall.exe for {target}"))?;
        let embedded_uninstaller = setup_source_dir.join("assets").join("uninstall.exe");
        fs::copy(&uninstaller, &embedded_uninstaller).with_context(|| {
            format!(
                "failed to copy {} to {}",
                uninstaller.display(),
                embedded_uninstaller.display()
            )
        })?;
        let embedded_uninstaller = fs::canonicalize(&embedded_uninstaller).with_context(|| {
            format!(
                "failed to find copied uninstall.exe at {}",
                embedded_uninstaller.display()
            )
        })?;

        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let target_manifest = platform
            .targets
            .iter()
            .find(|target_manifest| target_manifest.target == target)
            .context("failed to find target manifest")?;

        fs::write(setup_source_dir.join("build.rs"), SETUP_BUILD_RS)
            .with_context(|| "failed to write setup build.rs with embedded uninstaller payload")?;

        self.write_core_project(setup_source_dir)?;
        self.write_setup_build_input(
            platform,
            &target_manifest.files,
            Some(&embedded_uninstaller),
            setup_source_dir,
        )?;

        fs::write(setup_source_dir.join("src").join("setup.rs"), SETUP_RS)
            .with_context(|| "failed to write setup.rs with embedded uninstaller")?;

        Ok(())
    }

    fn write_core_project(&self, setup_source_dir: &Path) -> Result<()> {
        let core_dir = setup_source_dir.join("windows-installer-core");
        let core_src_dir = core_dir.join("src");
        let assets_dir = setup_source_dir.join("assets");
        fs::create_dir_all(&core_src_dir)
            .with_context(|| format!("failed to create {}", core_src_dir.display()))?;
        fs::create_dir_all(&assets_dir)
            .with_context(|| format!("failed to create {}", assets_dir.display()))?;
        fs::write(core_dir.join("Cargo.toml"), CORE_CARGO_TOML)
            .with_context(|| "failed to write installer core Cargo.toml")?;
        fs::write(core_src_dir.join("lib.rs"), CORE_RS)
            .with_context(|| "failed to write installer core lib.rs")?;
        fs::write(core_src_dir.join("config.rs"), CORE_CONFIG_RS)
            .with_context(|| "failed to write installer core config.rs")?;
        fs::write(core_src_dir.join("cli.rs"), CORE_CLI_RS)
            .with_context(|| "failed to write installer core cli.rs")?;
        fs::write(core_src_dir.join("install.rs"), CORE_INSTALL_RS)
            .with_context(|| "failed to write installer core install.rs")?;
        fs::write(core_src_dir.join("registry.rs"), CORE_REGISTRY_RS)
            .with_context(|| "failed to write installer core registry.rs")?;
        fs::write(core_src_dir.join("uninstall.rs"), CORE_UNINSTALL_RS)
            .with_context(|| "failed to write installer core uninstall.rs")?;

        Ok(())
    }

    fn write_setup_build_input(
        &self,
        platform: &PlatformManifest,
        files: &[PayloadFile],
        uninstaller_source: Option<&Path>,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let setup_config = SetupInstallerConfig::new(
            &self.build_manifest.app_name,
            &self.build_manifest.version,
            self.build_manifest.build.publisher.as_deref(),
            &platform.icons,
            &platform.variables,
            uninstaller_source,
            files,
        )?;

        fs::write(
            setup_source_dir.join(SETUP_CONFIG),
            serde_json::to_string_pretty(&setup_config)
                .context("failed to serialize setup config")?,
        )
        .with_context(|| "failed to write setup config")?;

        Ok(())
    }

    fn build_setup(&self, target: &str, setup_source_dir: &Path) -> Result<()> {
        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(target)
            .arg("--bin")
            .arg("setup")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build setup.exe for {target}"))?;

        if !status.success() {
            bail!("setup.exe build failed for {target}");
        }

        Ok(())
    }

    fn copy_setup_output(
        &self,
        target: &str,
        setup_source_dir: &Path,
        output_file: &Path,
    ) -> Result<()> {
        copy_release_exe(target, setup_source_dir, output_file, "setup")
    }

    fn clean_setup_source(&self, setup_source_dir: &Path) -> Result<()> {
        fs::remove_dir_all(setup_source_dir).with_context(|| {
            format!(
                "failed to remove generated setup project {}",
                setup_source_dir.display()
            )
        })
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn copy_release_exe(
    target: &str,
    setup_source_dir: &Path,
    output_file: &Path,
    name: &str,
) -> Result<()> {
    let source = release_exe_path(target, setup_source_dir, name);
    let destination = if output_file.extension().is_some() {
        output_file.to_path_buf()
    } else {
        output_file.join(format!("{name}.exe"))
    };

    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn release_exe_path(target: &str, setup_source_dir: &Path, name: &str) -> PathBuf {
    setup_source_dir
        .join("target")
        .join(target)
        .join("release")
        .join(format!("{name}.exe"))
}

fn setup_cargo_toml() -> String {
    SETUP_CARGO_TOML
        .replace(GENERATED_WORKSPACE_PLACEHOLDER, "[workspace]")
        .replace(
            GENERATED_CORE_DEPENDENCY_PLACEHOLDER,
            GENERATED_CORE_DEPENDENCY,
        )
}

fn write_setup_assets(setup_source_dir: &Path) -> Result<()> {
    let assets_dir = setup_source_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("failed to create {}", assets_dir.display()))?;
    fs::write(assets_dir.join("install.ico"), INSTALL_ICON)
        .with_context(|| "failed to write setup install icon")?;

    Ok(())
}

#[derive(Debug, Serialize)]
struct SetupInstallerConfig {
    app_name: String,
    app_version: String,
    publisher: Option<String>,
    variables: Vec<String>,
    uninstaller_source: String,
    payload: Vec<SetupPayloadFile>,
    icons: Vec<IconMapping>,
}

#[derive(Debug, Serialize)]
struct SetupPayloadFile {
    source: String,
    destination: String,
    executable: bool,
}

impl SetupInstallerConfig {
    fn new(
        app_name: &str,
        app_version: &str,
        publisher: Option<&str>,
        icons: &[IconMapping],
        variables: &[BuildVariable],
        uninstaller_source: Option<&Path>,
        files: &[PayloadFile],
    ) -> Result<Self> {
        Ok(Self {
            app_name: app_name.to_owned(),
            app_version: app_version.to_owned(),
            publisher: publisher.map(str::to_owned),
            variables: variables.iter().map(ToString::to_string).collect(),
            uninstaller_source: uninstaller_source
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            payload: files
                .iter()
                .map(SetupPayloadFile::try_from)
                .collect::<Result<Vec<_>>>()?,
            icons: icons.to_vec(),
        })
    }
}

impl TryFrom<&PayloadFile> for SetupPayloadFile {
    type Error = anyhow::Error;

    fn try_from(file: &PayloadFile) -> Result<Self> {
        let source = fs::canonicalize(&file.source)
            .with_context(|| format!("failed to find payload source {}", &file.source))?;

        Ok(Self {
            source: source.display().to_string(),
            destination: file.destination.clone(),
            executable: file.executable,
        })
    }
}

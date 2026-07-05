use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use image::ImageReader;
use minijinja::{Environment, context};
use serde_json::{Value, json};

use crate::services::build_manifest::BuildManifest;
use crate::services::manifest_file::{EulaFile, WindowsInstaller};
use crate::services::payload_file::PayloadFile;
use crate::services::platform_manifest::PlatformManifest;

const SETUP_CARGO_TOML: &str = include_str!("../../assets/windows-installer/Cargo.toml.j2");
const SETUP_BUILD_RS: &str = include_str!("../../assets/windows-installer/build.rs.j2");
const SETUP_RS: &str = include_str!("../../assets/windows-installer/main.rs.j2");
const UNINSTALL_RS: &str = include_str!("../../assets/windows-installer/uninstall.rs.j2");
const INSTALL_ICON: &[u8] = include_bytes!("../../assets/windows-installer/install.ico");
const WINDOWS_INSTALLER_VERSION: &str = "0.2.0";
const SETUP_CONFIG: &str = "setup-config.json";
const DISPLAY_ICON_SIZE: u32 = 256;

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

        fs::write(
            setup_source_dir.join("Cargo.toml"),
            setup_cargo_toml(platform)?,
        )
        .with_context(|| "failed to write setup Cargo.toml")?;
        fs::write(
            setup_source_dir.join("build.rs"),
            render_static_template("build.rs.j2", SETUP_BUILD_RS)?,
        )
        .with_context(|| "failed to write setup build.rs")?;
        self.write_setup_build_input(platform, files, None, setup_source_dir)?;
        write_setup_assets(setup_source_dir, platform.display_icon_source.as_deref())?;
        fs::write(
            setup_source_dir.join("src").join("main.rs"),
            render_setup_template("main.rs.j2", SETUP_RS, platform)?,
        )
        .with_context(|| "failed to write setup main.rs")?;
        fs::write(
            setup_source_dir.join("src").join("uninstall.rs"),
            render_setup_template("uninstall.rs.j2", UNINSTALL_RS, platform)?,
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

        fs::write(
            setup_source_dir.join("build.rs"),
            render_static_template("build.rs.j2", SETUP_BUILD_RS)?,
        )
        .with_context(|| "failed to write setup build.rs with embedded uninstaller payload")?;

        self.write_setup_build_input(
            platform,
            &target_manifest.files,
            Some(&embedded_uninstaller),
            setup_source_dir,
        )?;

        Ok(())
    }

    fn write_setup_build_input(
        &self,
        platform: &PlatformManifest,
        files: &[PayloadFile],
        uninstaller_source: Option<&Path>,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let setup_config = json!({
            "app_name": self.build_manifest.app_name,
            "app_version": self.build_manifest.version,
            "display_name": self.build_manifest.build.display_name,
            "publisher": self.build_manifest.build.publisher,
            "variables": platform.variables.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "uninstaller_source": uninstaller_source
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            "payload": files
                .iter()
                .map(setup_payload_file)
                .collect::<Result<Vec<_>>>()?,
            "display_icon": platform.display_icon,
            "display_icon_rgba": setup_display_icon(platform.display_icon_source.as_deref())?,
            "associated_files": platform.associated_files,
            "eulas": platform.eulas
                .iter()
                .map(setup_eula_file)
                .collect::<Result<Vec<_>>>()?,
        });

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

fn setup_cargo_toml(platform: &PlatformManifest) -> Result<String> {
    let dependencies = windows_installer_dependency(platform)?;
    let environment = Environment::new();
    let template = environment
        .template_from_str(SETUP_CARGO_TOML)
        .context("failed to parse setup Cargo.toml template")?;

    template
        .render(context! {
            windows_installer_dependency => dependencies.runtime,
            windows_installer_build_dependency => dependencies.build,
        })
        .context("failed to render setup Cargo.toml template")
}

fn render_static_template(name: &str, source: &str) -> Result<String> {
    let environment = Environment::new();
    let template = environment
        .template_from_str(source)
        .with_context(|| format!("failed to parse {name} template"))?;

    template
        .render(context! {})
        .with_context(|| format!("failed to render {name} template"))
}

fn render_setup_template(name: &str, source: &str, platform: &PlatformManifest) -> Result<String> {
    let environment = Environment::new();
    let template = environment
        .template_from_str(source)
        .with_context(|| format!("failed to parse {name} template"))?;

    template
        .render(context! {
            gui_installer => matches!(
                platform.installer.unwrap_or_default(),
                WindowsInstaller::Gui
            ),
        })
        .with_context(|| format!("failed to render {name} template"))
}

fn windows_installer_dependency(platform: &PlatformManifest) -> Result<WindowsInstallerDependency> {
    let cargoapp_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let windows_installer_dir = cargoapp_dir
        .parent()
        .context("failed to find cargoapp package parent")?
        .join("windows-installer");
    let source = if windows_installer_dir.is_dir() {
        let windows_installer_dir = fs::canonicalize(&windows_installer_dir)
            .with_context(|| format!("failed to find {}", windows_installer_dir.display()))?;

        format!(r#"path = "{}""#, windows_installer_dir.display())
    } else {
        format!(r#"version = "{WINDOWS_INSTALLER_VERSION}""#)
    };
    let feature = platform.installer.unwrap_or_default().cargo_feature();

    Ok(WindowsInstallerDependency {
        runtime: format!(r#"{{ {source}, default-features = false, features = ["{feature}"] }}"#),
        build: format!(r#"{{ {source}, default-features = false }}"#),
    })
}

struct WindowsInstallerDependency {
    runtime: String,
    build: String,
}

fn write_setup_assets(setup_source_dir: &Path, display_icon_source: Option<&str>) -> Result<()> {
    let assets_dir = setup_source_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("failed to create {}", assets_dir.display()))?;
    let install_icon = assets_dir.join("install.ico");
    if let Some(display_icon_source) = display_icon_source.filter(|source| {
        Path::new(source)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ico"))
    }) {
        fs::copy(display_icon_source, &install_icon).with_context(|| {
            format!(
                "failed to copy display icon {} to {}",
                display_icon_source,
                install_icon.display()
            )
        })?;
    } else {
        fs::write(&install_icon, INSTALL_ICON)
            .with_context(|| "failed to write setup install icon")?;
    }

    Ok(())
}

fn setup_payload_file(file: &PayloadFile) -> Result<Value> {
    let source = fs::canonicalize(&file.source)
        .with_context(|| format!("failed to find payload source {}", &file.source))?;

    Ok(json!({
        "source": source.display().to_string(),
        "destination": file.destination,
        "executable": file.executable,
    }))
}

fn setup_eula_file(eula: &EulaFile) -> Result<Value> {
    let path = Path::new(eula.path());
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read EULA file {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| eula.path());

    Ok(json!({
        "name": name,
        "text": text,
        "required": eula.required(),
    }))
}

fn setup_display_icon(source: Option<&str>) -> Result<Option<Value>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let path = Path::new(source);
    let supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("ico") || extension.eq_ignore_ascii_case("png")
        });

    if !supported {
        return Ok(None);
    }

    let icon = ImageReader::open(path)
        .with_context(|| format!("failed to open display icon {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode display icon {}", path.display()))?
        .resize_exact(
            DISPLAY_ICON_SIZE,
            DISPLAY_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();

    Ok(Some(json!({
        "width": DISPLAY_ICON_SIZE,
        "height": DISPLAY_ICON_SIZE,
        "rgba": icon.into_raw(),
    })))
}

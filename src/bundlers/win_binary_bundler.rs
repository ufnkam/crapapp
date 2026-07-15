use crate::build_manifest::BuildManifest;
use crate::bundlers::WindowsInstallerKind;
use crate::manifest_file::{AssociatedFileKind as ManifestAssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;
use crate::platform_manifests::WindowsPlatformManifest;
use crate::target_manifest::{Shortcut, TargetManifest};
use crate::windows_installer::{
    AssociatedFile as InstallerAssociatedFile, AssociatedFileKind as InstallerAssociatedFileKind,
    DisplayIcon, Eula as InstallerEula, InstallerConfig, PayloadEntry,
    Shortcut as InstallerShortcut,
};
use anyhow::{Context, bail};
use image::ImageReader;
use minijinja::{Environment, context};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SETUP_CARGO_TOML_TEMPLATE: &str =
    include_str!("../../assets/windows-installer/Cargo.toml.j2");
const INSTALLER_RS: &str = include_str!("../../assets/windows-installer/main.rs.j2");
const UNINSTALLER_RS: &str = include_str!("../../assets/windows-installer/uninstall.rs.j2");
const BUILD_RS_TEMPLATE: &str = include_str!("../../assets/windows-installer/build.rs.j2");

const BIN_ICON: &[u8] = include_bytes!("../../assets/windows-installer/install.ico");
const SETUP_CONFIG: &str = "setup-config.json";
const DISPLAY_ICON_SIZE: u32 = 256;

fn init_cargo_project(
    project_dir: &PathBuf,
    installer_kind: &WindowsInstallerKind,
) -> anyhow::Result<()> {
    let jinja_env = Environment::new();
    let src_path = project_dir.join("src");
    fs::create_dir_all(&src_path)?;

    // build.rs
    let build_rs_path = project_dir.join("build.rs");
    let build_rs = gen_build_rs(&jinja_env).with_context(|| "Failed to generate build rs")?;

    // cargo.toml
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let cargo_toml = gen_cargo_toml(&jinja_env, installer_kind)?;

    // installer source
    let installer_rs_path = src_path.join("main.rs");
    let installer_src = gen_source_file(INSTALLER_RS, &jinja_env, installer_kind)?;

    // uninstaller source
    let uninstaller_rs_path = src_path.join("uninstall.rs");
    let uninstaller_src = gen_source_file(UNINSTALLER_RS, &jinja_env, installer_kind)?;

    // save
    fs::write(build_rs_path, build_rs)?;
    fs::write(cargo_toml_path, cargo_toml)?;
    fs::write(installer_rs_path, installer_src)?;
    fs::write(uninstaller_rs_path, uninstaller_src)?;

    Ok(())
}

fn gen_build_rs(env: &Environment) -> anyhow::Result<String> {
    let build_rs = env.template_from_str(BUILD_RS_TEMPLATE)?;
    let build_rs = build_rs
        .render(context! {})
        .with_context(|| "failed to render buid.rs")?;
    Ok(build_rs)
}
fn gen_cargo_toml(
    env: &Environment,
    installer_kind: &WindowsInstallerKind,
) -> anyhow::Result<String> {
    let cargo_toml = env.template_from_str(SETUP_CARGO_TOML_TEMPLATE)?;
    let libcrapapp_version = env!("CARGO_PKG_VERSION");
    let libcrapapp_dep = libcrapapp_dependency(installer_kind, libcrapapp_version)
        .with_context(|| "Failed to add libcrapapp dependency")?;
    let cargo_toml = cargo_toml
        .render(context! {
            libcrapapp_dependency => libcrapapp_dep.runtime,
            libcrapapp_build_dependency => libcrapapp_dep.build,
        })
        .with_context(|| "failed to render Cargo.toml")?;
    Ok(cargo_toml)
}

fn gen_source_file(
    asset_path: &str,
    env: &Environment,
    installer_kind: &WindowsInstallerKind,
) -> anyhow::Result<String> {
    let template = env.template_from_str(asset_path)?;
    let res = template.render(context! {
        gui_installer => matches!(installer_kind, WindowsInstallerKind::Gui),
    })?;

    Ok(res)
}

fn prepare_bundler(
    build_dir: &Path,
    platform: &str,
    target: &str,
    installer_kind: &WindowsInstallerKind,
) -> anyhow::Result<PathBuf> {
    let project_dir = build_dir
        .join(platform)
        .join(target)
        .join(installer_kind.to_string());

    if project_dir.exists() {
        fs::remove_dir_all(&project_dir)?
    }
    fs::create_dir_all(&project_dir).with_context(|| {
        format!(
            "Failed to create build directory at {}",
            project_dir.display()
        )
    })?;

    let src_dir = project_dir.join("builder");

    init_cargo_project(&src_dir, installer_kind)?;
    gen_setup_assets(&src_dir.join("../assets"))?;

    Ok(project_dir)
}

pub struct LibcrapappDependency {
    pub runtime: String,
    pub build: String,
}

pub fn libcrapapp_dependency(
    installer_kind: &WindowsInstallerKind,
    libcrapapp_version: &str,
) -> anyhow::Result<LibcrapappDependency> {
    #[cfg(feature = "dev")]
    let source = {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest_dir = fs::canonicalize(manifest_dir)
            .with_context(|| format!("failed to find {}", manifest_dir.display()))?;

        format!(
            r#"package = "cargo-crapapp", path = "{}""#,
            manifest_dir.display()
        )
    };

    #[cfg(not(feature = "dev"))]
    let source = format!(r#"package = "cargo-crapapp", version = "{libcrapapp_version}""#);

    let feature = installer_kind.cargo_feature();

    Ok(LibcrapappDependency {
        runtime: format!(r#"{{ {source}, default-features = false, features = ["{feature}"] }}"#),
        build: format!(r#"{{ {source}, default-features = false }}"#),
    })
}

pub fn gen_setup_assets(assets_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("failed to create {}", assets_dir.display()))?;
    let install_icon = assets_dir.join("install.ico");

    fs::write(&install_icon, BIN_ICON).with_context(|| "failed to write setup install icon")?;

    Ok(())
}

pub fn gen_installer_config(
    build_manifest: &BuildManifest,
    platform: &WindowsPlatformManifest<TargetManifest>,
    files: &[PayloadFile],
    shortcuts: &[Shortcut],
    uninstaller_source: Option<&Path>,
    output_path: &Path,
) -> anyhow::Result<()> {
    let setup_config = InstallerConfig {
        app_name: build_manifest.app_name.clone(),
        app_version: build_manifest.version.clone(),
        display_name: build_manifest.build.display_name.clone(),
        publisher: build_manifest.build.publisher.clone(),
        required_variables: platform.variables.iter().map(ToString::to_string).collect(),
        uninstaller_source: uninstaller_source
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        uninstaller_bytes: &[],
        payload: files
            .iter()
            .map(setup_payload_file)
            .collect::<anyhow::Result<Vec<_>>>()?,
        display_icon: platform.display_icon.clone(),
        display_icon_rgba: setup_display_icon(platform.display_icon_source.as_deref())?,
        associated_files: platform
            .associated_files
            .iter()
            .map(setup_associated_file)
            .collect(),
        shortcuts: shortcuts.iter().map(setup_shortcut).collect(),
        eulas: platform
            .eulas
            .iter()
            .map(setup_eula_file)
            .collect::<anyhow::Result<Vec<_>>>()?,
    };

    fs::write(
        output_path,
        serde_json::to_string_pretty(&setup_config).context("failed to serialize setup config")?,
    )
    .with_context(|| "failed to write setup config")?;

    Ok(())
}

fn setup_payload_file(file: &PayloadFile) -> anyhow::Result<PayloadEntry> {
    let source = fs::canonicalize(&file.source)
        .with_context(|| format!("failed to find payload source {}", &file.source))?;

    Ok(PayloadEntry {
        source: Some(source.display().to_string()),
        destination: file.destination.clone(),
        executable: file.executable,
        offset: 0,
        len: 0,
        bytes: &[],
    })
}

fn setup_eula_file(eula: &EulaFile) -> anyhow::Result<InstallerEula> {
    let path = Path::new(eula.path());
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read EULA file {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| eula.path());

    Ok(InstallerEula {
        name: name.to_owned(),
        text,
        required: eula.required(),
    })
}

fn setup_shortcut(shortcut: &Shortcut) -> InstallerShortcut {
    InstallerShortcut {
        target: shortcut.target.clone(),
        name: shortcut.name.clone(),
        directory: shortcut.directory.clone(),
        icon: shortcut.icon.clone(),
    }
}

fn setup_associated_file(file: &crate::manifest_file::AssociatedFile) -> InstallerAssociatedFile {
    InstallerAssociatedFile {
        path: file.path.clone(),
        kind: match file.kind {
            ManifestAssociatedFileKind::File => InstallerAssociatedFileKind::File,
            ManifestAssociatedFileKind::Directory => InstallerAssociatedFileKind::Directory,
        },
        eula_report: file.eula_report,
    }
}

fn setup_display_icon(source: Option<&str>) -> anyhow::Result<Option<DisplayIcon>> {
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
        bail!(
            "display icon {} must be a .ico or .png file",
            path.display()
        );
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

    Ok(Some(DisplayIcon {
        width: DISPLAY_ICON_SIZE,
        height: DISPLAY_ICON_SIZE,
        rgba: icon.into_raw(),
    }))
}

fn build_bin(build_space: &Path, target: &str, bin: &str) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target-dir")
        .arg(build_space.join("../../target"))
        .arg("--target")
        .arg(target)
        .arg("--bin")
        .arg(bin)
        .current_dir(build_space)
        .status()?;

    if !status.success() {
        bail!("{bin}.exe build failed for {target}");
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

pub struct WinBinaryBundler {}
impl WinBinaryBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &WindowsPlatformManifest<TargetManifest>,
        target_manifest: &TargetManifest,
        inst_mode: &WindowsInstallerKind,
    ) -> anyhow::Result<()> {
        let target = &target_manifest.target;
        if target_manifest.files.is_empty() {
            bail!("windows bundle has no files to package");
        }

        let project_space =
            prepare_bundler(build_dir, &platform_manifest.platform, target, inst_mode)?;
        let builder_space = project_space.join("builder");
        let assets_dir = builder_space.join("../assets");
        let cargo_target_dir = builder_space.join("../../target");
        let target_dir = builder_space
            .join("../../target")
            .join(target)
            .join("release");
        let installer_bin = inst_mode.installer_binary_name();
        let uninstaller_bin = "uninstall.exe";

        gen_installer_config(
            build_manifest,
            platform_manifest,
            &target_manifest.files,
            &target_manifest.shortcuts,
            None,
            &builder_space.join(SETUP_CONFIG),
        )?;

        remove_dir_if_exists(&cargo_target_dir)?;
        build_bin(&builder_space, target, "uninstall")
            .with_context(|| format!("failed to build {} uninstaller", inst_mode))?;
        let embedded_uninstaller = assets_dir.join(uninstaller_bin);
        fs::copy(target_dir.join(uninstaller_bin), &embedded_uninstaller).with_context(|| {
            format!(
                "failed to copy {} to {}",
                target_dir.join(uninstaller_bin).display(),
                embedded_uninstaller.display()
            )
        })?;
        let embedded_uninstaller = fs::canonicalize(&embedded_uninstaller).with_context(|| {
            format!(
                "failed to find copied uninstall.exe at {}",
                embedded_uninstaller.display()
            )
        })?;

        gen_installer_config(
            build_manifest,
            platform_manifest,
            &target_manifest.files,
            &target_manifest.shortcuts,
            Some(&embedded_uninstaller),
            &builder_space.join(SETUP_CONFIG),
        )?;

        remove_dir_if_exists(&cargo_target_dir)?;
        build_bin(&builder_space, target, "setup")
            .with_context(|| format!("failed to build {} installer", inst_mode))?;
        fs::copy(
            target_dir.join(installer_bin),
            project_space.join(installer_bin),
        )
        .with_context(|| {
            format!(
                "failed to copy {} to {}",
                target_dir.join(installer_bin).display(),
                project_space.join(installer_bin).display()
            )
        })?;
        fs::remove_dir_all(builder_space)?;
        Ok(())
    }
}

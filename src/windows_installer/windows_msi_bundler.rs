use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use editpe::Image;

use crate::build_manifest::BuildManifest;
use crate::bundlers::{self, WindowsBundlerKind};
use crate::manifest_file::{AssociatedFileKind as ManifestAssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;
use crate::platform_manifests::WindowsPlatformManifest;
use crate::progress;
use crate::target_manifest::TargetManifest;
use crate::windows_installer::msi;
use crate::windows_installer::{
    AssociatedFile, AssociatedFileKind, DisplayIcon, Eula, InstallerConfig, PayloadEntry, Shortcut,
};
use image::ImageReader;

pub struct WindowsMsiBundler {}

const DISPLAY_ICON_SIZE: u32 = 256;
const MSI_PICKER_CARGO_TOML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/windows-msi-picker/Cargo.toml.template"
));
const MSI_PICKER_CARGO_LOCK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/windows-msi-picker/Cargo.lock"
));
const MSI_PICKER_LIB_RS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/windows-msi-picker/src/lib.rs"
));

impl WindowsMsiBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &WindowsPlatformManifest<TargetManifest>,
        target_manifest: &TargetManifest,
        bundle: &WindowsBundlerKind,
    ) -> anyhow::Result<()> {
        let target_dir = build_dir
            .join(&platform_manifest.platform)
            .join(&target_manifest.target)
            .join(bundle.to_string());

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("failed to remove {}", target_dir.display()))?;
        }
        fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;

        stamp_payload_icons(
            platform_manifest.display_icon_source.as_deref(),
            target_manifest,
        )?;
        let output = target_dir.join(format!("{}.msi", artifact_file_stem(build_manifest)));
        let plan = prepare_windows_bundle(build_manifest, platform_manifest, target_manifest)?;
        let folder_picker_action = build_folder_picker_action(build_dir, &target_manifest.target)?;

        msi::build(
            &plan,
            &output,
            &folder_picker_action,
            platform_manifest.display_icon_source.as_deref(),
        )
        .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

/// Stamps the configured application icon into every executable payload before
/// the executable is embedded in the MSI. This intentionally happens after
/// Cargo has produced the PE files and before any external code-signing step.
fn stamp_payload_icons(
    icon_source: Option<&str>,
    target_manifest: &TargetManifest,
) -> anyhow::Result<()> {
    let Some(icon_source) = icon_source else {
        return Ok(());
    };

    for file in target_manifest.files.iter().filter(|file| file.executable) {
        let executable = Path::new(&file.source);
        let mut image = Image::parse_file(executable).with_context(|| {
            format!("failed to read Windows executable {}", executable.display())
        })?;
        let mut resources = image.resource_directory().cloned().unwrap_or_default();
        resources.set_main_icon_file(icon_source).with_context(|| {
            format!(
                "failed to apply icon {icon_source} to {}",
                executable.display()
            )
        })?;
        image
            .set_resource_directory(resources)
            .with_context(|| format!("failed to update resources in {}", executable.display()))?;
        image.write_file(executable).with_context(|| {
            format!(
                "failed to write icon-stamped executable {}",
                executable.display()
            )
        })?;
    }

    Ok(())
}

/// Builds the Windows-only `cdylib` containing the MSI folder-picker action.
/// The resulting DLL is embedded into the MSI, never shipped beside it.
fn build_folder_picker_action(build_dir: &Path, target: &str) -> anyhow::Result<Vec<u8>> {
    let target_dir = build_dir
        .join("windows")
        .join(target)
        .join("msi-picker-target");
    let source_dir = build_dir
        .join("windows")
        .join(target)
        .join("msi-picker-source");
    write_folder_picker_crate(&source_dir)?;
    let manifest = source_dir.join("Cargo.toml");

    let status = progress::run(
        &format!("Building Windows folder picker for {target}"),
        || {
            Command::new("cargo")
                .args(["build", "--release", "--quiet", "--manifest-path"])
                .arg(&manifest)
                .args(["--target", target, "--target-dir"])
                .arg(&target_dir)
                .arg("--lib")
                .status()
        },
    )?;
    if !status.success() {
        bail!("failed to build the embedded Windows folder picker for {target}");
    }

    let action = target_dir
        .join(target)
        .join("release")
        .join("crapapp_msi_picker_action.dll");
    let action = fs::read(&action).with_context(|| {
        format!(
            "failed to read the embedded Windows folder picker at {}",
            action.display()
        )
    })?;
    fs::remove_dir_all(&target_dir).with_context(|| {
        format!(
            "failed to remove temporary MSI folder-picker build directory {}",
            target_dir.display()
        )
    })?;
    fs::remove_dir_all(&source_dir).with_context(|| {
        format!(
            "failed to remove temporary MSI folder-picker source directory {}",
            source_dir.display()
        )
    })?;

    Ok(action)
}

/// The picker crate is embedded into cargo-crapapp so an installed CLI never
/// relies on the source directory it was originally compiled from.
fn write_folder_picker_crate(source_dir: &Path) -> anyhow::Result<()> {
    if source_dir.exists() {
        fs::remove_dir_all(source_dir).with_context(|| {
            format!(
                "failed to remove temporary MSI folder-picker source directory {}",
                source_dir.display()
            )
        })?;
    }
    fs::create_dir_all(source_dir.join("src")).with_context(|| {
        format!(
            "failed to create temporary MSI folder-picker source directory {}",
            source_dir.display()
        )
    })?;
    fs::write(source_dir.join("Cargo.toml"), MSI_PICKER_CARGO_TOML)
        .context("failed to write temporary MSI folder-picker Cargo.toml")?;
    fs::write(source_dir.join("Cargo.lock"), MSI_PICKER_CARGO_LOCK)
        .context("failed to write temporary MSI folder-picker Cargo.lock")?;
    fs::write(source_dir.join("src/lib.rs"), MSI_PICKER_LIB_RS)
        .context("failed to write temporary MSI folder-picker source")?;
    Ok(())
}

fn artifact_file_stem(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .display_name
        .as_deref()
        .unwrap_or(&build_manifest.app_name)
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
}

fn prepare_windows_bundle(
    build_manifest: &BuildManifest,
    platform: &WindowsPlatformManifest<TargetManifest>,
    target: &TargetManifest,
) -> anyhow::Result<InstallerConfig> {
    Ok(InstallerConfig {
        app_name: build_manifest.app_name.clone(),
        app_version: build_manifest.version.clone(),
        display_name: build_manifest.build.display_name.clone(),
        publisher: build_manifest.build.publisher.clone(),
        bundled_at: build_manifest.bundled_at.clone(),
        bundle_target: target.target.clone(),
        required_variables: required_variables(platform),
        uninstaller_source: String::new(),
        uninstaller_bytes: &[],
        payload: target
            .files
            .iter()
            .map(payload_entry)
            .collect::<anyhow::Result<_>>()?,
        path_entries: platform.path_entries.clone(),
        display_icon: platform.display_icon.clone(),
        display_icon_rgba: display_icon(platform.display_icon_source.as_deref())?,
        associated_files: platform
            .associated_files
            .iter()
            .map(associated_file)
            .collect(),
        shortcuts: target.shortcuts.iter().map(shortcut).collect(),
        eulas: platform
            .eulas
            .iter()
            .map(eula)
            .collect::<anyhow::Result<_>>()?,
    })
}

fn required_variables(platform: &WindowsPlatformManifest<TargetManifest>) -> Vec<String> {
    let mut variables = platform
        .variables
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    // PATH entries are based on the selected MSI installation directory even
    // when the payload itself did not explicitly mention `$INSTALLPATH`.
    if !variables.iter().any(|variable| variable == "INSTALLPATH") {
        variables.push("INSTALLPATH".to_owned());
    }

    variables
}

fn payload_entry(file: &PayloadFile) -> anyhow::Result<PayloadEntry> {
    let source = fs::canonicalize(&file.source)
        .with_context(|| format!("failed to find payload source {}", file.source))?;
    Ok(PayloadEntry {
        source: Some(source.display().to_string()),
        destination: file.destination.clone(),
        executable: file.executable,
        offset: 0,
        len: 0,
        bytes: &[],
    })
}

fn eula(eula: &EulaFile) -> anyhow::Result<Eula> {
    let path = Path::new(eula.path());
    Ok(Eula {
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| eula.path())
            .to_owned(),
        text: fs::read_to_string(path)
            .with_context(|| format!("failed to read EULA file {}", path.display()))?,
        required: eula.required(),
    })
}

fn associated_file(file: &crate::manifest_file::AssociatedFile) -> AssociatedFile {
    AssociatedFile {
        path: file.path.clone(),
        kind: match file.kind {
            ManifestAssociatedFileKind::File => AssociatedFileKind::File,
            ManifestAssociatedFileKind::Directory => AssociatedFileKind::Directory,
        },
        eula_report: file.eula_report,
    }
}

fn shortcut(shortcut: &crate::target_manifest::Shortcut) -> Shortcut {
    Shortcut {
        target: shortcut.target.clone(),
        name: shortcut.name.clone(),
        directory: shortcut.directory.clone(),
        icon: shortcut.icon.clone(),
    }
}

fn display_icon(source: Option<&str>) -> anyhow::Result<Option<DisplayIcon>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let path = Path::new(source);
    if !bundlers::shared::path_has_extension(path, &["ico", "png"]) {
        bail!(
            "display icon {} must be a .ico or .png file",
            path.display()
        );
    }
    let image = ImageReader::open(path)?
        .decode()?
        .resize_exact(
            DISPLAY_ICON_SIZE,
            DISPLAY_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();
    Ok(Some(DisplayIcon {
        width: DISPLAY_ICON_SIZE,
        height: DISPLAY_ICON_SIZE,
        rgba: image.into_raw(),
    }))
}

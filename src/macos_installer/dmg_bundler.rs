use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};

use crate::build_manifest::BuildManifest;
use crate::bundlers::MacosBundlerKind;
use crate::macos_installer::app_bundler::{MacosAppBundler, bundle_name};
use crate::package_metadata::artifact_file_stem;
use crate::platform_manifests::MacosPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct MacosDmgBundler;

impl MacosDmgBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &MacosPlatformManifest,
        target_manifest: &TargetManifest,
        bundle: &MacosBundlerKind,
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

        let stage_dir = target_dir.join("stage");
        fs::create_dir_all(&stage_dir)
            .with_context(|| format!("failed to create {}", stage_dir.display()))?;
        let app_path = stage_dir.join(format!("{}.app", bundle_name(build_manifest)));
        MacosAppBundler::bundle_to_path(
            build_manifest,
            platform_manifest,
            target_manifest,
            &app_path,
        )?;
        create_applications_link(&stage_dir)?;

        let dmg_path = target_dir.join(format!("{}.dmg", artifact_file_stem(build_manifest)));
        if dmg_path.exists() {
            fs::remove_file(&dmg_path)
                .with_context(|| format!("failed to remove {}", dmg_path.display()))?;
        }

        let status = Command::new("hdiutil")
            .arg("create")
            .arg("-volname")
            .arg(crate::package_metadata::display_name(build_manifest))
            .arg("-srcfolder")
            .arg(&stage_dir)
            .arg("-fs")
            .arg("HFS+")
            .arg("-ov")
            .arg("-format")
            .arg("UDZO")
            .arg(&dmg_path)
            .status()
            .context("failed to run hdiutil to create macOS dmg")?;

        fs::remove_dir_all(&stage_dir)
            .with_context(|| format!("failed to remove {}", stage_dir.display()))?;

        if !status.success() {
            bail!("hdiutil failed to create {}", dmg_path.display());
        }

        Ok(())
    }
}

#[cfg(unix)]
fn create_applications_link(stage_dir: &Path) -> anyhow::Result<()> {
    std::os::unix::fs::symlink("/Applications", stage_dir.join("Applications"))
        .with_context(|| "failed to create Applications symlink in dmg staging directory")
}

#[cfg(not(unix))]
fn create_applications_link(_stage_dir: &Path) -> anyhow::Result<()> {
    Ok(())
}

use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::build_manifest::BuildManifest;
use crate::bundlers::WindowsBundlerKind;
use crate::platform_manifests::WindowsPlatformManifest;
use crate::target_manifest::TargetManifest;
use crate::windows_installer::msi::{self, MsiSpec};

pub struct WindowsMsiBundler {}

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

        let output = target_dir.join(format!("{}.msi", artifact_file_stem(build_manifest)));
        let spec = MsiSpec {
            package: build_manifest.app_name.clone(),
            display_name: build_manifest
                .build
                .display_name
                .clone()
                .unwrap_or_else(|| build_manifest.app_name.clone()),
            version: build_manifest.version.clone(),
            manufacturer: build_manifest
                .build
                .publisher
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            files: target_manifest.files.clone(),
            associated_files: platform_manifest.associated_files.clone(),
            shortcuts: target_manifest.shortcuts.clone(),
            display_icon: platform_manifest.display_icon.clone(),
            display_icon_source: platform_manifest.display_icon_source.clone(),
        };

        msi::build(&spec, &output)
            .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

fn artifact_file_stem(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .display_name
        .as_deref()
        .unwrap_or(&build_manifest.app_name)
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
}

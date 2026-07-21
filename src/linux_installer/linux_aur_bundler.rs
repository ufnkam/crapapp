use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxBundlerKind;
use crate::linux_installer::aur::{self, AurSpec};
use crate::package_metadata::{description, package_name};
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct LinuxAurBundler {}

impl LinuxAurBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &LinuxPlatformManifest<TargetManifest>,
        target_manifest: &TargetManifest,
        bundle: &LinuxBundlerKind,
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

        let package = package_name(&build_manifest.app_name);
        let output = target_dir.join(format!("{package}.src.tar.gz"));
        let architecture = if target_manifest.target.starts_with("aarch64-") {
            "aarch64"
        } else {
            "x86_64"
        };
        let spec = AurSpec {
            package,
            version: build_manifest.version.clone(),
            bundled_at: build_manifest.bundled_at.clone(),
            description: description(build_manifest),
            architecture: architecture.to_owned(),
            files: target_manifest.files.clone(),
            associated_files: platform_manifest.associated_files.clone(),
            eulas: platform_manifest.eulas.clone(),
        };

        aur::build(&spec, &output)
            .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxInstallerKind;
use crate::linux_package::{package_name, rpm_architecture};
use crate::linux_rpm::{self, RpmSpec};
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct LinuxRpmBundler {}

impl LinuxRpmBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &LinuxPlatformManifest<TargetManifest>,
        target_manifest: &TargetManifest,
        bundle: &LinuxInstallerKind,
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
        let output = target_dir.join(format!("{package}-{}-1.rpm", build_manifest.version));
        let description = build_manifest
            .build
            .display_name
            .clone()
            .unwrap_or_else(|| build_manifest.app_name.clone());
        let spec = RpmSpec {
            package,
            version: build_manifest.version.clone(),
            release: "1".to_owned(),
            summary: description.clone(),
            description,
            architecture: rpm_architecture(&target_manifest.target)?.to_owned(),
            license: "custom".to_owned(),
            files: target_manifest.files.clone(),
            associated_files: platform_manifest.associated_files.clone(),
            eulas: platform_manifest.eulas.clone(),
        };

        linux_rpm::build(&spec, &output)
            .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

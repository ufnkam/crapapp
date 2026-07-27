use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxBundlerKind;
use crate::linux_installer::deb::{self, DebSpec};
use crate::linux_installer::prepare_payload;
use crate::package_metadata::{description, homepage, summary};
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct LinuxDebBundler {}

impl LinuxDebBundler {
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

        let payload = prepare_payload(build_manifest, platform_manifest, target_manifest)?;
        let output = target_dir.join(format!("{}.deb", payload.package));
        let architecture = if target_manifest.target.starts_with("aarch64-") {
            "arm64"
        } else {
            "amd64"
        };
        let spec = DebSpec {
            package: payload.package,
            version: build_manifest.version.clone(),
            bundled_at: build_manifest.bundled_at.clone(),
            maintainer: build_manifest
                .build
                .publisher
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            summary: summary(build_manifest),
            description: description(build_manifest),
            homepage: homepage(build_manifest).map(str::to_owned),
            architecture: architecture.to_owned(),
            files: payload.files,
            generated_files: payload.generated_files,
            associated_files: platform_manifest.associated_files.clone(),
            eulas: platform_manifest.eulas.clone(),
        };

        deb::build(&spec, &output)
            .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

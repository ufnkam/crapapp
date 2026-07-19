use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxInstallerKind;
use crate::linux_deb::{self, DebSpec};
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct LinuxDebBundler {}

impl LinuxDebBundler {
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

        let package_name = package_name(&build_manifest.app_name);
        let output = target_dir.join(format!("{package_name}.deb"));
        let spec = DebSpec {
            package: package_name,
            version: build_manifest.version.clone(),
            maintainer: build_manifest
                .build
                .publisher
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            description: build_manifest
                .build
                .display_name
                .clone()
                .unwrap_or_else(|| build_manifest.app_name.clone()),
            architecture: deb_architecture(&target_manifest.target)?.to_owned(),
            files: target_manifest.files.clone(),
            associated_files: platform_manifest.associated_files.clone(),
            eulas: platform_manifest.eulas.clone(),
        };

        linux_deb::build(&spec, &output)
            .with_context(|| format!("failed to write {}", output.display()))?;

        Ok(())
    }
}

fn package_name(name: &str) -> String {
    let mut package = String::new();
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.') {
            package.push(character);
        } else {
            package.push('-');
        }
    }

    let package = package.trim_matches(['-', '.']).to_owned();
    if package.is_empty() {
        "app".to_owned()
    } else {
        package
    }
}

fn deb_architecture(target: &str) -> anyhow::Result<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => Ok("amd64"),
        _ => bail!("deb architecture mapping for target {target} is not supported yet"),
    }
}

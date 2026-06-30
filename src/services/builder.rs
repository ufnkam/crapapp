use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::services::build_manifest::BuildManifest;
use crate::services::windows_bundler::WindowsBundler;

pub struct Builder<'a> {
    build_manifest: &'a BuildManifest,
}

impl<'a> Builder<'a> {
    pub fn new(build_manifest: &'a BuildManifest) -> Self {
        Self { build_manifest }
    }

    pub fn build(&self) -> Result<()> {
        let build_root = build_root()?;
        let build_dir = build_root.join(".crapapp_build");
        reset_build_dir(&build_dir)?;

        for platform in &self.build_manifest.platforms {
            for target in &platform.targets {
                let mut command = Command::new("cargo");
                command.current_dir(&build_root);
                command.arg("build").arg("--release");
                command.arg("--target-dir").arg(&build_root.join("target"));
                command.arg("--target").arg(&target.target);

                for package in &self.build_manifest.build.packages {
                    command.arg("--package").arg(package);
                }

                if !self.build_manifest.build.features.is_empty() {
                    command
                        .arg("--features")
                        .arg(self.build_manifest.build.features.join(" "));
                }

                let status = command
                    .status()
                    .with_context(|| format!("failed to run cargo build for {}", target.target))?;

                if !status.success() {
                    bail!("cargo build failed for {}", target.target);
                }
            }
        }

        if self
            .build_manifest
            .platforms
            .iter()
            .any(|platform| platform.platform == "windows")
        {
            WindowsBundler::new(self.build_manifest, &build_dir).bundle()?;
        }

        Ok(())
    }
}

fn build_root() -> Result<PathBuf> {
    std::env::current_dir().context("failed to resolve current directory")
}

fn reset_build_dir(build_dir: &Path) -> Result<()> {
    if build_dir.exists() {
        fs::remove_dir_all(build_dir)
            .with_context(|| format!("failed to remove {}", build_dir.display()))?;
    }

    fs::create_dir_all(build_dir)
        .with_context(|| format!("failed to create {}", build_dir.display()))?;

    Ok(())
}

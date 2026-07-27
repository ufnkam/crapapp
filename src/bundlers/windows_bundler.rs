use std::path::Path;

use crate::platform_manifests::WindowsPlatformManifest;
use crate::progress;
use crate::target_manifest::TargetManifest;
use crate::windows_installer::windows_msi_bundler::WindowsMsiBundler;
use crate::{build_manifest::BuildManifest, bundlers::bundler_kinds::WindowsBundlerKind};

pub struct WindowsBundler<'a> {
    build_manifest: &'a BuildManifest,
    platform: &'a WindowsPlatformManifest<TargetManifest>,
    build_dir: &'a Path,
}

impl<'a> WindowsBundler<'a> {
    pub fn new(
        build_manifest: &'a BuildManifest,
        platform: &'a WindowsPlatformManifest<TargetManifest>,
        build_dir: &'a Path,
    ) -> Self {
        Self {
            build_manifest,
            platform,
            build_dir,
        }
    }

    pub fn bundle(&self, bundles: &[WindowsBundlerKind]) -> anyhow::Result<()> {
        for target in &self.platform.targets {
            for bundle in bundles {
                let message = format!("Bundling windows {} {}", target.target, bundle);
                progress::run(&message, || {
                    WindowsMsiBundler::bundle(
                        self.build_manifest,
                        self.build_dir,
                        self.platform,
                        target,
                        bundle,
                    )
                })?;
            }
        }

        Ok(())
    }
}

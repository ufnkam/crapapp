use std::path::Path;

use crate::bundlers::win_binary_bundler::WinBinaryBundler;
use crate::bundlers::windows_msi_bundler::WindowsMsiBundler;
use crate::platform_manifests::WindowsPlatformManifest;
use crate::target_manifest::TargetManifest;
use crate::{build_manifest::BuildManifest, bundlers::windows_installer::WindowsInstallerKind};

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

    pub fn bundle(&self) -> anyhow::Result<()> {
        for target in &self.platform.targets {
            for bundle in &self.platform.bundle {
                match bundle {
                    WindowsInstallerKind::Cli | WindowsInstallerKind::Gui => {
                        WinBinaryBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    WindowsInstallerKind::Msi => {
                        WindowsMsiBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}

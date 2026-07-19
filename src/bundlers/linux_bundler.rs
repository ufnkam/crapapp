use std::path::Path;

use anyhow::bail;

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxInstallerKind;
use crate::bundlers::linux_deb_bundler::LinuxDebBundler;
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub struct LinuxBundler<'a> {
    build_manifest: &'a BuildManifest,
    platform: &'a LinuxPlatformManifest<TargetManifest>,
    build_dir: &'a Path,
}

impl<'a> LinuxBundler<'a> {
    pub fn new(
        build_manifest: &'a BuildManifest,
        platform: &'a LinuxPlatformManifest<TargetManifest>,
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
                    LinuxInstallerKind::Deb => {
                        LinuxDebBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    LinuxInstallerKind::Rpm => {
                        bail!("rpm bundle support is not implemented yet");
                    }
                    LinuxInstallerKind::Aur => {
                        bail!("aur bundle support is not implemented yet");
                    }
                }
            }
        }

        Ok(())
    }
}

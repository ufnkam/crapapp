use std::path::Path;

use crate::build_manifest::BuildManifest;
use crate::bundlers::LinuxInstallerKind;
use crate::bundlers::linux_aur_bundler::LinuxAurBundler;
use crate::bundlers::linux_deb_bundler::LinuxDebBundler;
use crate::bundlers::linux_rpm_bundler::LinuxRpmBundler;
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
                        LinuxRpmBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    LinuxInstallerKind::Aur => {
                        LinuxAurBundler::bundle(
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

use std::path::Path;

use crate::build_manifest::BuildManifest;
use crate::bundlers::bundler_kinds::LinuxBundlerKind;
use crate::linux_installer::linux_aur_bundler::LinuxAurBundler;
use crate::linux_installer::linux_deb_bundler::LinuxDebBundler;
use crate::linux_installer::linux_rpm_bundler::LinuxRpmBundler;
use crate::platform_manifests::LinuxPlatformManifest;
use crate::progress;
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

    pub fn bundle(&self, bundles: &[LinuxBundlerKind]) -> anyhow::Result<()> {
        for target in &self.platform.targets {
            for bundle in bundles {
                let message = format!("Bundling linux {} {}", target.target, bundle);
                progress::run(&message, || match bundle {
                    LinuxBundlerKind::Deb => {
                        LinuxDebBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                        Ok::<(), anyhow::Error>(())
                    }
                    LinuxBundlerKind::Rpm => {
                        LinuxRpmBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                        Ok::<(), anyhow::Error>(())
                    }
                    LinuxBundlerKind::Aur => {
                        LinuxAurBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                        Ok::<(), anyhow::Error>(())
                    }
                })?;
            }
        }

        Ok(())
    }
}

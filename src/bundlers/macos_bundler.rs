use std::path::Path;

use anyhow::bail;

use crate::build_manifest::BuildManifest;
use crate::bundlers::MacosInstallerKind;
use crate::bundlers::macos_app_bundler::MacosAppBundler;
use crate::bundlers::macos_pkg_bundler::MacosPkgBundler;
use crate::platform_manifests::MacosPlatformManifest;

pub struct MacosBundler<'a> {
    build_manifest: &'a BuildManifest,
    platform: &'a MacosPlatformManifest,
    build_dir: &'a Path,
}

impl<'a> MacosBundler<'a> {
    pub fn new(
        build_manifest: &'a BuildManifest,
        platform: &'a MacosPlatformManifest,
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
                    MacosInstallerKind::App => {
                        MacosAppBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    MacosInstallerKind::Pkg => {
                        MacosPkgBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    MacosInstallerKind::Dmg => {
                        bail!("dmg bundle support is not implemented yet");
                    }
                }
            }
        }

        Ok(())
    }
}

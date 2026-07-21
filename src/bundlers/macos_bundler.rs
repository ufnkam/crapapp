use std::path::Path;

use crate::build_manifest::BuildManifest;
use crate::bundlers::bundler_kinds::MacosBundlerKind;
use crate::macos_installer::app_bundler::MacosAppBundler;
use crate::macos_installer::dmg_bundler::MacosDmgBundler;
use crate::macos_installer::pkg_bundler::MacosPkgBundler;
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
                    MacosBundlerKind::App => {
                        MacosAppBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    MacosBundlerKind::Pkg => {
                        MacosPkgBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                    }
                    MacosBundlerKind::Dmg => {
                        MacosDmgBundler::bundle(
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

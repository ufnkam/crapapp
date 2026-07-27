use std::path::Path;

use crate::build_manifest::BuildManifest;
use crate::bundlers::bundler_kinds::MacosBundlerKind;
use crate::macos_installer::app_bundler::MacosAppBundler;
use crate::macos_installer::dmg_bundler::MacosDmgBundler;
use crate::macos_installer::pkg_bundler::MacosPkgBundler;
use crate::platform_manifests::MacosPlatformManifest;
use crate::progress;

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

    pub fn bundle(&self, bundles: &[MacosBundlerKind]) -> anyhow::Result<()> {
        for target in &self.platform.targets {
            for bundle in bundles {
                let message = format!("Bundling macos {} {}", target.target, bundle);
                progress::run(&message, || match bundle {
                    MacosBundlerKind::App => {
                        MacosAppBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                        Ok::<(), anyhow::Error>(())
                    }
                    MacosBundlerKind::Pkg => {
                        MacosPkgBundler::bundle(
                            self.build_manifest,
                            self.build_dir,
                            self.platform,
                            target,
                            bundle,
                        )?;
                        Ok::<(), anyhow::Error>(())
                    }
                    MacosBundlerKind::Dmg => {
                        MacosDmgBundler::bundle(
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

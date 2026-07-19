use crate::bundlers::MacosInstallerKind;
use serde::{Deserialize, Serialize};

use crate::manifest_file::EulaFile;
use crate::platform_manifest::PlatformManifest;
use crate::target_manifest::TargetManifest;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MacosPlatformManifest<Target = TargetManifest> {
    pub platform: String,
    pub targets: Vec<Target>,
    pub bundle: Vec<MacosInstallerKind>,
    pub pkg: MacosPkgConfig,
    pub eulas: Vec<EulaFile>,
    pub display_icon: Option<String>,
    pub app_binary: Option<String>,
    #[serde(skip, default)]
    pub display_icon_source: Option<String>,
}

impl MacosPlatformManifest<TargetManifest> {
    pub fn new(
        targets: Vec<TargetManifest>,
        display_icon: Option<&str>,
        display_icon_source: Option<&str>,
        app_binary: Option<&str>,
        bundle: Vec<MacosInstallerKind>,
        pkg: MacosPkgConfig,
        eulas: Vec<EulaFile>,
    ) -> Self {
        Self {
            platform: "macos".to_owned(),
            targets,
            bundle,
            pkg,
            eulas,
            display_icon: display_icon.map(str::to_owned),
            app_binary: app_binary.map(str::to_owned),
            display_icon_source: display_icon_source.map(str::to_owned),
        }
    }
}

impl PlatformManifest for MacosPlatformManifest<TargetManifest> {
    fn platform(&self) -> &str {
        &self.platform
    }

    fn targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.target.as_str())
            .collect()
    }

    fn write_text(&self, output: &mut String) {
        output.push_str(&format!("  {}\n", self.platform()));

        if let Some(display_icon) = &self.display_icon {
            output.push_str(&format!("    display icon: {display_icon}\n"));
        }

        if !self.bundle.is_empty() {
            let bundles = self
                .bundle
                .iter()
                .map(|bundle| bundle.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!("    bundle: {bundles}\n"));
        }

        if let Some(app_binary) = &self.app_binary {
            output.push_str(&format!("    app binary: {app_binary}\n"));
        }

        if let Some(identifier) = &self.pkg.identifier {
            output.push_str(&format!("    pkg identifier: {identifier}\n"));
        }

        if let Some(install_path) = &self.pkg.install_path {
            output.push_str(&format!("    pkg install path: {install_path}\n"));
        }

        if self.pkg.link_bins {
            output.push_str(&format!("    pkg bin dir: {}\n", self.pkg.bin_dir()));
        }

        if !self.eulas.is_empty() {
            output.push_str("    eulas:\n");

            for eula in &self.eulas {
                let required = if eula.required() {
                    "required"
                } else {
                    "optional"
                };
                output.push_str(&format!("      {} ({required})\n", eula.path()));
            }
        }

        for target in &self.targets {
            output.push_str(&format!("    {}\n", target.target));

            for file in &target.files {
                let marker = if file.executable { "x" } else { "-" };
                output.push_str(&format!(
                    "      [{}] {} -> {}\n",
                    marker, file.source, file.destination
                ));
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MacosPkgConfig {
    pub identifier: Option<String>,
    pub install_path: Option<String>,
    pub bin_dir: Option<String>,
    #[serde(default = "default_link_bins")]
    pub link_bins: bool,
}

impl Default for MacosPkgConfig {
    fn default() -> Self {
        Self {
            identifier: None,
            install_path: None,
            bin_dir: None,
            link_bins: true,
        }
    }
}

impl MacosPkgConfig {
    pub fn bin_dir(&self) -> &str {
        self.bin_dir.as_deref().unwrap_or("/usr/local/bin")
    }
}

fn default_link_bins() -> bool {
    true
}

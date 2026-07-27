use crate::bundlers::LinuxBundlerKind;
use crate::manifest_file::{
    AssociatedFile, EulaFile, FileMapping, LinuxTarget, PlatformManifest as SourcePlatformManifest,
    ShortcutMapping,
};
use crate::platform_manifest::PlatformManifest;
use crate::target_manifest::TargetManifest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    deserialize = "Target: Deserialize<'de>",
    serialize = "Target: Serialize"
))]
pub struct LinuxPlatformManifest<Target = LinuxTarget> {
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(
        default,
        deserialize_with = "crate::manifest_file::deserialize_linux_bundles"
    )]
    pub bundle: Vec<LinuxBundlerKind>,
    pub install_path: Option<String>,
    pub bin_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileMapping>,
    #[serde(default)]
    pub associated_files: Vec<AssociatedFile>,
    #[serde(default)]
    pub eulas: Vec<EulaFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortcuts: Vec<ShortcutMapping>,
    pub display_icon: Option<String>,
}

impl<Target> LinuxPlatformManifest<Target> {
    pub fn bundles(&self) -> Vec<LinuxBundlerKind> {
        let bundles = if self.bundle.is_empty() {
            vec![LinuxBundlerKind::Deb]
        } else {
            self.bundle.clone()
        };
        let mut unique_bundles = Vec::with_capacity(bundles.len());

        for bundle in bundles {
            if !unique_bundles.contains(&bundle) {
                unique_bundles.push(bundle);
            }
        }

        unique_bundles
    }

    pub fn bin_dir(&self) -> &str {
        self.bin_dir.as_deref().unwrap_or("/usr/bin")
    }

    pub fn install_path(&self) -> Option<&str> {
        self.install_path.as_deref()
    }

    pub fn variable_sources(&self) -> Vec<&str> {
        self.install_path.iter().map(String::as_str).collect()
    }

    pub fn display_icon(&self) -> Option<&str> {
        self.display_icon.as_deref()
    }
}

impl LinuxPlatformManifest<LinuxTarget> {
    pub fn build(
        &self,
        targets: Vec<TargetManifest>,
        display_icon: Option<&str>,
    ) -> LinuxPlatformManifest<TargetManifest> {
        LinuxPlatformManifest {
            platform: "linux".to_owned(),
            targets,
            bundle: self.bundles(),
            install_path: self.install_path.clone(),
            bin_dir: self.bin_dir.clone(),
            files: Vec::new(),
            associated_files: self.associated_files.clone(),
            eulas: self.eulas.clone(),
            shortcuts: self.shortcuts.clone(),
            display_icon: display_icon.map(str::to_owned),
        }
    }
}

impl SourcePlatformManifest for LinuxPlatformManifest {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn bin_dir(&self) -> &str {
        self.bin_dir()
    }

    fn install_path(&self) -> Option<&str> {
        self.install_path()
    }

    fn variable_sources(&self) -> Vec<&str> {
        self.variable_sources()
    }

    fn files(&self) -> &[FileMapping] {
        &self.files
    }

    fn display_icon(&self) -> Option<&str> {
        self.display_icon()
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(LinuxTarget::target).collect()
    }
}

impl PlatformManifest for LinuxPlatformManifest<TargetManifest> {
    fn platform(&self) -> &str {
        if self.platform.is_empty() {
            "linux"
        } else {
            &self.platform
        }
    }

    fn targets(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|target| target.target.as_str())
            .collect()
    }

    fn write_text(&self, output: &mut String) {
        output.push_str(&format!("  {}\n", self.platform()));

        if !self.bundle.is_empty() {
            let bundles = self
                .bundle
                .iter()
                .map(|bundle| bundle.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!("    bundle: {bundles}\n"));
        }

        if let Some(install_path) = &self.install_path {
            output.push_str(&format!("    install path: {install_path}\n"));
        }

        output.push_str(&format!("    bin dir: {}\n", self.bin_dir()));

        if let Some(display_icon) = &self.display_icon {
            output.push_str(&format!("    display icon: {display_icon}\n"));
        }

        if !self.associated_files.is_empty() {
            output.push_str("    associated files:\n");

            for file in &self.associated_files {
                let eula_report = if file.eula_report {
                    " (EULA report)"
                } else {
                    ""
                };
                output.push_str(&format!(
                    "      {:?}: {}{}\n",
                    file.kind, file.path, eula_report
                ));
            }
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

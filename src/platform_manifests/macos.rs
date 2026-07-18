use crate::bundlers::MacosInstallerKind;
use serde::Serialize;

use crate::platform_manifest::PlatformManifest;
use crate::target_manifest::TargetManifest;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MacosPlatformManifest<Target = TargetManifest> {
    pub platform: String,
    pub targets: Vec<Target>,
    pub bundle: Vec<MacosInstallerKind>,
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
    ) -> Self {
        Self {
            platform: "macos".to_owned(),
            targets,
            bundle,
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

use crate::services::build_variable::{BuildVariable, platform_variables};
use crate::services::manifest_file::WindowsInstaller;
use crate::services::payload_file::PayloadFile;
use crate::services::target_manifest::TargetManifest;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PlatformManifest {
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<WindowsInstaller>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_icon: Option<String>,
    pub variables: Vec<BuildVariable>,
    pub targets: Vec<TargetManifest>,
}

impl PlatformManifest {
    pub fn new(
        platform: &str,
        installer: Option<WindowsInstaller>,
        display_icon: Option<&str>,
        variable_sources: &[&str],
        files: &[PayloadFile],
        targets: Vec<TargetManifest>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            platform: platform.to_owned(),
            installer,
            display_icon: display_icon.map(str::to_owned),
            variables: platform_variables(variable_sources, files)?,
            targets,
        })
    }
}

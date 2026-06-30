use crate::services::build_variable::{BuildVariable, platform_variables};
use crate::services::icons::IconMapping;
use crate::services::payload_file::PayloadFile;
use crate::services::target_manifest::TargetManifest;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PlatformManifest {
    pub platform: String,
    pub variables: Vec<BuildVariable>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<IconMapping>,
    pub targets: Vec<TargetManifest>,
}

impl PlatformManifest {
    pub fn new(
        platform: &str,
        variable_sources: &[&str],
        files: &[PayloadFile],
        icons: Vec<IconMapping>,
        targets: Vec<TargetManifest>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            platform: platform.to_owned(),
            variables: platform_variables(variable_sources, files)?,
            icons,
            targets,
        })
    }
}

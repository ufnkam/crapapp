use serde::Serialize;

use crate::services::manifest_file::CrapManifest;

#[derive(Debug, Serialize)]
pub struct BuildConfigManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    pub packages: Vec<String>,
    pub features: Vec<String>,
}

impl BuildConfigManifest {
    pub fn from_crap_manifest(manifest: &CrapManifest) -> Self {
        let Some(build) = &manifest.build else {
            return Self {
                publisher: None,
                packages: Vec::new(),
                features: Vec::new(),
            };
        };

        Self {
            publisher: build.publisher.clone(),
            packages: build.packages.clone(),
            features: build.features.clone(),
        }
    }
}

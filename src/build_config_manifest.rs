use serde::Serialize;

use crate::manifest_file::CrapManifest;

#[derive(Debug, Serialize)]
pub struct BuildConfigManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub packages: Vec<String>,
    pub features: Vec<String>,
}

impl BuildConfigManifest {
    pub fn from_crap_manifest(manifest: &CrapManifest, cargo_description: Option<String>) -> Self {
        let Some(build) = &manifest.build else {
            return Self {
                publisher: None,
                display_name: None,
                description: cargo_description,
                packages: Vec::new(),
                features: Vec::new(),
            };
        };

        Self {
            publisher: build.publisher.clone(),
            display_name: build.display_name.clone(),
            description: build.description.clone().or(cargo_description),
            packages: build.packages.clone(),
            features: build.features.clone(),
        }
    }
}

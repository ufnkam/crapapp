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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_file: Option<String>,
    pub packages: Vec<String>,
    pub features: Vec<String>,
}

impl BuildConfigManifest {
    pub fn from_crap_manifest(
        manifest: &CrapManifest,
        cargo_description: Option<String>,
        cargo_homepage: Option<String>,
        cargo_license: Option<String>,
        cargo_license_file: Option<String>,
    ) -> Self {
        let Some(build) = &manifest.build else {
            return Self {
                publisher: None,
                display_name: None,
                description: cargo_description,
                homepage: cargo_homepage,
                license: cargo_license,
                license_file: cargo_license_file,
                packages: Vec::new(),
                features: Vec::new(),
            };
        };

        Self {
            publisher: build.publisher.clone(),
            display_name: build.display_name.clone(),
            description: build.description.clone().or(cargo_description),
            homepage: build.homepage.clone().or(cargo_homepage),
            license: build.license.clone().or(cargo_license),
            license_file: build.license_file.clone().or(cargo_license_file),
            packages: build.packages.clone(),
            features: build.features.clone(),
        }
    }
}

#![allow(dead_code)]

use crate::build_manifest::BuildManifest;

pub fn package_name(name: &str) -> String {
    let mut package = String::new();
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.') {
            package.push(character);
        } else {
            package.push('-');
        }
    }

    let package = package.trim_matches(['-', '.']).to_owned();
    if package.is_empty() {
        "app".to_owned()
    } else {
        package
    }
}

pub fn display_name(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&build_manifest.app_name)
        .to_owned()
}

pub fn publisher(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|publisher| !publisher.is_empty())
        .unwrap_or("unknown")
        .to_owned()
}

pub fn homepage(build_manifest: &BuildManifest) -> Option<&str> {
    build_manifest
        .build
        .homepage
        .as_deref()
        .filter(|homepage| !homepage.trim().is_empty())
}

pub fn license(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .license
        .as_deref()
        .filter(|license| !license.trim().is_empty())
        .unwrap_or("LicenseRef-proprietary")
        .to_owned()
}

pub fn description(build_manifest: &BuildManifest) -> String {
    build_manifest
        .build
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .unwrap_or_else(|| {
            build_manifest
                .build
                .display_name
                .as_deref()
                .map(str::trim)
                .filter(|display_name| !display_name.is_empty())
                .unwrap_or(&build_manifest.app_name)
        })
        .to_owned()
}

pub fn summary(build_manifest: &BuildManifest) -> String {
    display_name(build_manifest)
}

pub fn artifact_file_stem(build_manifest: &BuildManifest) -> String {
    display_name(build_manifest).replace(['/', '\\', ':'], "-")
}

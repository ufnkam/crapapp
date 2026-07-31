use uuid::Uuid;

use crate::windows_installer::InstallerConfig;

const DEFAULT_MANUFACTURER: &str = "unknown";
const COMPONENT_64BIT: i32 = 256;

pub(super) fn display_name(config: &InstallerConfig) -> &str {
    config.display_name.as_deref().unwrap_or(&config.app_name)
}

pub(super) fn manufacturer(config: &InstallerConfig) -> &str {
    config.publisher_name().unwrap_or(DEFAULT_MANUFACTURER)
}

pub(super) fn cabinet_stream_name(package: &str) -> String {
    let mut name = package
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();

    if name.is_empty() {
        name.push_str("app");
    }
    name.push_str(".cab");
    name
}

pub(super) fn identifier(value: &str) -> String {
    let id = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();

    if id.is_empty() { "Item".to_owned() } else { id }
}

pub(super) fn package_code(plan: &InstallerConfig) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v3:package:{}:{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target, plan.bundled_at
        )
        .as_bytes(),
    )
}

pub(super) fn product_code(plan: &InstallerConfig) -> Uuid {
    code(
        "product",
        format!(
            "{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target
        ),
    )
}

pub(super) fn upgrade_code(plan: &InstallerConfig) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("cargo-crapapp:msi:upgrade:{}", plan.app_name).as_bytes(),
    )
}

pub(super) fn component_code(plan: &InstallerConfig, file_id: &str) -> Uuid {
    code(
        "component",
        format!(
            "{}:{}:{}:{file_id}",
            plan.app_name, plan.app_version, plan.bundle_target
        ),
    )
}

pub(super) fn shortcut_component_code(plan: &InstallerConfig, shortcut_id: &str) -> Uuid {
    code(
        "shortcut-component",
        format!(
            "{}:{}:{}:{shortcut_id}",
            plan.app_name, plan.app_version, plan.bundle_target
        ),
    )
}

pub(super) fn path_component_code(plan: &InstallerConfig, path_id: &str) -> Uuid {
    code(
        "path-component",
        format!(
            "{}:{}:{}:{path_id}",
            plan.app_name, plan.app_version, plan.bundle_target
        ),
    )
}

fn code(kind: &str, value: String) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("cargo-crapapp:msi:v2:{kind}:{value}").as_bytes(),
    )
}

pub(super) fn msi_version(version: &str) -> String {
    let mut parts = version.split('.').take(3).collect::<Vec<_>>();
    while parts.len() < 3 {
        parts.push("0");
    }
    parts.join(".")
}

pub(super) fn msi_platform(target: &str) -> &'static str {
    match target.split_once('-').map(|(architecture, _)| architecture) {
        Some("aarch64") => "Arm64",
        Some("x86_64") => "x64",
        _ => "Intel",
    }
}

pub(super) fn msi_page_count(target: &str) -> i32 {
    if target.starts_with("aarch64-") {
        500
    } else if target.starts_with("x86_64-") {
        200
    } else {
        100
    }
}

pub(super) fn component_attributes(target: &str) -> i32 {
    if target.starts_with("aarch64-") || target.starts_with("x86_64-") {
        COMPONENT_64BIT
    } else {
        0
    }
}

#![allow(dead_code)]

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};

use crate::build_manifest::BuildManifest;
use crate::linux_installer::desktop_entry::{application_icons, desktop_entries};
use crate::package_metadata::package_name;
use crate::platform_manifests::LinuxPlatformManifest;
use crate::target_manifest::TargetManifest;

pub mod aur;
pub mod deb;
pub mod desktop_entry;
pub mod linux_aur_bundler;
pub mod linux_deb_bundler;
pub mod linux_rpm_bundler;
pub mod rpm;

/// A file synthesized while creating a Linux package, rather than copied from
/// the application's payload.
pub struct GeneratedFile {
    pub install_path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

pub struct PreparedPayload {
    pub package: String,
    pub architecture: &'static str,
    pub files: Vec<crate::payload_file::PayloadFile>,
    pub generated_files: Vec<GeneratedFile>,
}

pub fn prepare_payload(
    build_manifest: &BuildManifest,
    platform_manifest: &LinuxPlatformManifest<TargetManifest>,
    target_manifest: &TargetManifest,
) -> anyhow::Result<PreparedPayload> {
    let package = package_name(&build_manifest.app_name);
    let icons = platform_manifest
        .display_icon
        .as_deref()
        .map(|source| application_icons(source, &package))
        .transpose()?;
    let appstream_id = appstream_id(build_manifest);
    let mut desktop_entries = desktop_entries(
        build_manifest,
        target_manifest,
        icons.as_ref().map(|_| package.as_str()),
    )?;
    if let Some((desktop_id, _)) = desktop_entries.first_mut() {
        *desktop_id = appstream_id.clone();
    }
    let mut generated_files = desktop_entries
        .iter()
        .map(|(id, contents)| GeneratedFile {
            install_path: format!("/usr/share/applications/{id}"),
            bytes: contents.as_bytes().to_vec(),
            executable: false,
        })
        .collect::<Vec<_>>();
    if let Some((desktop_id, _)) = desktop_entries.first() {
        generated_files.push(GeneratedFile {
            install_path: format!("/usr/share/metainfo/{appstream_id}.metainfo.xml"),
            bytes: appstream_metadata(
                build_manifest,
                &appstream_id,
                desktop_id,
                icons.as_ref().map(|_| package.as_str()),
            )
            .into_bytes(),
            executable: false,
        });
    }
    if let Some(icons) = icons {
        generated_files.extend(
            icons
                .into_iter()
                .map(|(install_path, bytes)| GeneratedFile {
                    install_path,
                    bytes,
                    executable: false,
                }),
        );
    }
    generated_files.push(GeneratedFile {
        install_path: format!("/usr/share/doc/{package}/copyright"),
        bytes: debian_copyright(build_manifest).into_bytes(),
        executable: false,
    });
    if let Some(source) = build_manifest.build.license_file.as_deref() {
        let bytes = std::fs::read(source)
            .with_context(|| format!("failed to read license_file {source}"))?;
        generated_files.push(GeneratedFile {
            install_path: format!("/usr/share/licenses/{package}/LICENSE"),
            bytes,
            executable: false,
        });
    }

    Ok(PreparedPayload {
        package,
        architecture: if target_manifest.target.starts_with("aarch64-") {
            "aarch64"
        } else {
            "x86_64"
        },
        files: target_manifest.files.clone(),
        generated_files,
    })
}

fn appstream_id(build_manifest: &BuildManifest) -> String {
    let publisher = crate::package_metadata::publisher(build_manifest);
    let publisher = publisher
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    let publisher = if publisher.is_empty() {
        "application".to_owned()
    } else {
        publisher
    };
    format!(
        "com.{publisher}.{}.desktop",
        package_name(&build_manifest.app_name)
    )
}

fn appstream_metadata(
    build_manifest: &BuildManifest,
    appstream_id: &str,
    desktop_id: &str,
    icon_name: Option<&str>,
) -> String {
    let name = crate::package_metadata::display_name(build_manifest);
    let summary = build_manifest
        .build
        .description
        .as_deref()
        .filter(|description| !description.trim().is_empty())
        .unwrap_or("Application packaged by cargo-crapapp");
    let icon = if let Some(icon_name) = icon_name {
        format!("\n  <icon type=\"stock\">{}</icon>", xml_escape(icon_name))
    } else {
        String::new()
    };
    let developer = crate::package_metadata::publisher(build_manifest);
    let homepage = build_manifest
        .build
        .homepage
        .as_deref()
        .filter(|homepage| !homepage.trim().is_empty())
        .map(|homepage| format!("\n  <url type=\"homepage\">{}</url>", xml_escape(homepage)))
        .unwrap_or_default();
    let license = build_manifest
        .build
        .license
        .as_deref()
        .filter(|license| !license.trim().is_empty())
        .unwrap_or("LicenseRef-proprietary");

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<component type=\"desktop-application\">\n  <id>{}</id>\n  <pkgname>{}</pkgname>\n  <name>{}</name>\n  <summary>{}</summary>\n  <developer_name>{}</developer_name>\n  <metadata_license>CC0-1.0</metadata_license>\n  <project_license>{}</project_license>{}{}\n  <launchable type=\"desktop-id\">{}</launchable>\n  <provides><id>{}</id></provides>\n  <description><p>{}</p></description>\n</component>\n",
        xml_escape(appstream_id),
        xml_escape(&package_name(&build_manifest.app_name)),
        xml_escape(&name),
        xml_escape(summary),
        xml_escape(&developer),
        xml_escape(license),
        icon,
        homepage,
        xml_escape(desktop_id),
        xml_escape(desktop_id),
        xml_escape(summary),
    )
}

/// PackageKit reads this standard Debian copyright manifest when presenting a
/// local `.deb` in Ubuntu App Center.  It is separate from AppStream metadata:
/// App Center asks PackageKit for the license before the package is installed.
fn debian_copyright(build_manifest: &BuildManifest) -> String {
    let name = crate::package_metadata::display_name(build_manifest);
    let publisher = crate::package_metadata::publisher(build_manifest);
    let license = license_id(build_manifest);
    let source = build_manifest
        .build
        .homepage
        .as_deref()
        .filter(|homepage| !homepage.trim().is_empty())
        .map(|homepage| format!("Source: {homepage}\n"))
        .unwrap_or_default();

    format!(
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nUpstream-Name: {name}\n{source}Files: *\nCopyright: {publisher}\nLicense: {license}\n"
    )
}

fn license_id(build_manifest: &BuildManifest) -> &str {
    build_manifest
        .build
        .license
        .as_deref()
        .filter(|license| !license.trim().is_empty())
        .unwrap_or("LicenseRef-proprietary")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn install_relative_path(path: &str) -> anyhow::Result<String> {
    if path.trim().is_empty() {
        bail!("Linux install path must not be empty");
    }

    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => relative.push(part),
            Component::ParentDir | Component::Prefix(_) => {
                bail!("Linux install path {path} must not contain parent or prefix components");
            }
        }
    }

    if relative.as_os_str().is_empty() {
        bail!("Linux install path {path} must not resolve to package root");
    }

    Ok(relative.display().to_string())
}

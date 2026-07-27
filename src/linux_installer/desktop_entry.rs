use anyhow::bail;
use image::{ImageFormat, ImageReader};
use std::io::Cursor;

use crate::package_metadata::display_name;
use crate::{build_manifest::BuildManifest, target_manifest::TargetManifest};

pub fn desktop_entries(
    build_manifest: &BuildManifest,
    target_manifest: &TargetManifest,
    icon: Option<&str>,
) -> anyhow::Result<Vec<(String, String)>> {
    if !target_manifest.shortcuts.is_empty() {
        return target_manifest
            .shortcuts
            .iter()
            .map(|shortcut| {
                let binary = std::path::Path::new(&shortcut.target)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Linux shortcut target {} must have a UTF-8 file name",
                            shortcut.target
                        )
                    })?;
                let id = desktop_id(&shortcut.name, binary)?;
                let icon = shortcut.icon.as_deref().or(icon);
                Ok((
                    id,
                    entry(build_manifest, &shortcut.name, &shortcut.target, icon),
                ))
            })
            .collect();
    }

    let executable_count = target_manifest
        .files
        .iter()
        .filter(|file| file.executable)
        .count();
    let mut entries = Vec::new();
    for file in target_manifest.files.iter().filter(|file| file.executable) {
        let binary = std::path::Path::new(&file.destination)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Linux executable destination {} must have a UTF-8 file name",
                    file.destination
                )
            })?;
        let base_name = display_name(build_manifest);
        let name = if executable_count > 1 {
            format!("{base_name} ({binary})")
        } else {
            base_name
        };
        let id = desktop_id(&name, binary)?;
        entries.push((id, entry(build_manifest, &name, binary, icon)));
    }
    Ok(entries)
}

fn entry(
    build_manifest: &BuildManifest,
    name: &str,
    executable: &str,
    icon: Option<&str>,
) -> String {
    let mut entry = String::new();
    entry.push_str("[Desktop Entry]\n");
    entry.push_str("Type=Application\n");
    entry.push_str(&format!("Name={}\n", escape_value(name)));
    entry.push_str(&format!(
        "Comment={}\n",
        escape_value(&crate::package_metadata::description(build_manifest))
    ));
    entry.push_str(&format!("Exec={}\n", escape_exec(executable)));
    if let Some(icon) = icon {
        entry.push_str(&format!("Icon={}\n", escape_value(icon)));
    }
    entry.push_str("Terminal=false\n");
    entry.push_str("Categories=Utility;\n");
    entry
}

pub fn application_icons(source: &str, package: &str) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let source_image = ImageReader::open(source)?.decode()?;
    let package = crate::package_metadata::package_name(package);
    [64_u32, 128, 256]
        .into_iter()
        .map(|size| {
            let image =
                source_image.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
            let mut bytes = Vec::new();
            image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)?;
            Ok((
                format!("/usr/share/icons/hicolor/{size}x{size}/apps/{package}.png"),
                bytes,
            ))
        })
        .collect()
}

fn desktop_id(name: &str, binary: &str) -> anyhow::Result<String> {
    if binary.trim().is_empty() {
        bail!("Linux desktop entry binary name cannot be empty");
    }
    let app = name
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
    Ok(format!(
        "{}-{binary}.desktop",
        if app.is_empty() { "app" } else { &app }
    ))
}

fn escape_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn escape_exec(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        build_config_manifest::BuildConfigManifest,
        build_manifest::BuildManifest,
        payload_file::PayloadFile,
        target_manifest::{Shortcut, TargetManifest},
    };
    use image::{ImageBuffer, Rgba};

    use super::{application_icons, desktop_entries};

    #[test]
    fn desktop_entries_use_shortcut_names_targets_and_icons() {
        let target = TargetManifest {
            target: "x86_64-unknown-linux-gnu".to_owned(),
            files: vec![PayloadFile::executable(
                "first".to_owned(),
                "/usr/bin/first".to_owned(),
            )],
            shortcuts: vec![Shortcut {
                target: "/usr/bin/first".to_owned(),
                name: "First App".to_owned(),
                directory: Some("Example".to_owned()),
                icon: Some("/usr/share/icons/first.png".to_owned()),
            }],
        };
        let manifest = BuildManifest {
            app_name: "example".to_owned(),
            version: "1.0.0".to_owned(),
            bundled_at: String::new(),
            build: BuildConfigManifest {
                publisher: None,
                display_name: Some("Example App".to_owned()),
                description: None,
                homepage: None,
                license: None,
                license_file: None,
                packages: Vec::new(),
                features: Vec::new(),
            },
            platforms: Vec::new(),
        };

        let entries = desktop_entries(&manifest, &target, Some("fallback-icon"))
            .expect("desktop entry should be generated");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].1.contains("Name=First App\n"));
        assert!(entries[0].1.contains("Exec=/usr/bin/first\n"));
        assert!(entries[0].1.contains("Icon=/usr/share/icons/first.png\n"));
    }

    #[test]
    fn application_icon_is_standard_png() {
        let directory =
            std::env::temp_dir().join(format!("cargo-crapapp-desktop-icon-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temporary directory should be created");
        let source = directory.join("source.png");
        ImageBuffer::<Rgba<u8>, _>::from_pixel(2, 2, Rgba([10, 20, 30, 255]))
            .save(&source)
            .expect("test icon should be written");

        let icons =
            application_icons(&source.display().to_string(), "Example App").expect("icon converts");
        assert_eq!(icons.len(), 3);
        for (size, (destination, bytes)) in [64, 128, 256].into_iter().zip(icons) {
            assert_eq!(
                destination,
                format!("/usr/share/icons/hicolor/{size}x{size}/apps/example-app.png")
            );
            let image = image::load_from_memory(&bytes).expect("generated icon must be PNG");
            assert_eq!(image.width(), size);
            assert_eq!(image.height(), size);
        }

        fs::remove_dir_all(directory).expect("temporary directory should be removed");
    }
}

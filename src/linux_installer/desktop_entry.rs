use anyhow::bail;

use crate::package_metadata::display_name;
use crate::{build_manifest::BuildManifest, target_manifest::TargetManifest};

pub fn desktop_entries(
    build_manifest: &BuildManifest,
    target_manifest: &TargetManifest,
    icon: Option<&str>,
) -> anyhow::Result<Vec<(String, String)>> {
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
        let name = display_name(build_manifest);
        let id = desktop_id(&name, binary)?;
        let mut entry = String::new();
        entry.push_str("[Desktop Entry]\n");
        entry.push_str("Type=Application\n");
        entry.push_str(&format!("Name={}\n", escape_value(&name)));
        entry.push_str(&format!("Exec={binary}\n"));
        if let Some(icon) = icon {
            entry.push_str(&format!("Icon={icon}\n"));
        }
        entry.push_str("Terminal=false\n");
        entry.push_str("Categories=Utility;\n");
        entries.push((id, entry));
    }
    Ok(entries)
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

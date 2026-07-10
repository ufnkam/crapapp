use serde::{Deserialize, Serialize};
use std::path::Path;

use anyhow::{Result, bail};

use crate::manifest_file::ShortcutMapping;
use crate::payload_file::{PayloadFile, resolve_destination};

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetManifest {
    pub target: String,
    pub files: Vec<PayloadFile>,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shortcut {
    pub target: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl TargetManifest {
    pub fn new(
        target: &str,
        binary_names: &[String],
        install_path: Option<&str>,
        bin_dir: &str,
        extra_files: &[PayloadFile],
        shortcut_mappings: &[ShortcutMapping],
    ) -> Result<Self> {
        let mut files = binary_names
            .iter()
            .map(|binary| {
                let binary_file_name = binary_file_name(target, binary);

                PayloadFile::executable(
                    Path::new("target")
                        .join(target)
                        .join("release")
                        .join(&binary_file_name)
                        .display()
                        .to_string(),
                    resolve_destination(
                        install_path,
                        &Path::new(bin_dir)
                            .join(&binary_file_name)
                            .display()
                            .to_string(),
                    ),
                )
            })
            .collect::<Vec<_>>();

        let shortcut_icon_files = shortcut_mappings
            .iter()
            .filter_map(|shortcut| shortcut.icon.as_deref())
            .map(|icon| shortcut_icon_payload(icon, install_path, bin_dir))
            .collect::<Result<Vec<_>>>()?;

        for icon_file in shortcut_icon_files {
            push_payload_file(&mut files, icon_file)?;
        }

        for file in extra_files.iter().cloned() {
            push_payload_file(&mut files, file)?;
        }
        let shortcuts = shortcut_mappings
            .iter()
            .map(|shortcut| {
                if !binary_names.iter().any(|binary| binary == &shortcut.binary) {
                    bail!("shortcut references unknown binary {}", shortcut.binary);
                }

                let binary_file_name = binary_file_name(target, &shortcut.binary);

                Ok(Shortcut {
                    target: resolve_destination(
                        install_path,
                        &Path::new(bin_dir)
                            .join(&binary_file_name)
                            .display()
                            .to_string(),
                    ),
                    name: shortcut.name.clone(),
                    directory: shortcut.directory.clone(),
                    icon: shortcut
                        .icon
                        .as_deref()
                        .map(|icon| shortcut_icon_destination(icon, install_path, bin_dir))
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            target: target.to_owned(),
            files,
            shortcuts,
        })
    }
}

fn shortcut_icon_payload(source: &str, install_path: Option<&str>, bin_dir: &str) -> Result<PayloadFile> {
    let source_path = Path::new(source);

    if !source_path.is_file() {
        bail!("shortcut icon source {} does not exist", source_path.display());
    }

    Ok(PayloadFile::data(
        source.to_owned(),
        shortcut_icon_destination(source, install_path, bin_dir)?,
    ))
}

fn shortcut_icon_destination(
    source: &str,
    install_path: Option<&str>,
    bin_dir: &str,
) -> Result<String> {
    let source_path = Path::new(source);
    let file_name = source_path.file_name().ok_or_else(|| {
        anyhow::anyhow!(
            "shortcut icon source {} must point to a file",
            source_path.display()
        )
    })?;

    Ok(resolve_destination(
        install_path,
        &Path::new(bin_dir).join(file_name).display().to_string(),
    ))
}

fn push_payload_file(files: &mut Vec<PayloadFile>, payload: PayloadFile) -> Result<()> {
    if let Some(existing) = files.iter().find(|file| file.destination == payload.destination) {
        if existing.source != payload.source || existing.executable != payload.executable {
            bail!(
                "payload destination {} is produced by both {} and {}",
                payload.destination,
                existing.source,
                payload.source
            );
        }

        return Ok(());
    }

    files.push(payload);
    Ok(())
}

fn binary_file_name(target: &str, binary_name: &str) -> String {
    if target.contains("windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::TargetManifest;
    use crate::manifest_file::ShortcutMapping;

    #[test]
    fn shortcut_icon_is_added_to_payload_and_shortcut() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-shortcut-icon-{}.ico",
            std::process::id()
        ));
        std::fs::write(&icon_path, b"icon").expect("failed to write icon");

        let manifest = TargetManifest::new(
            "x86_64-pc-windows-gnu",
            &[String::from("example")],
            Some("$INSTALLPATH"),
            "bin",
            &[],
            &[ShortcutMapping {
                binary: "example".to_owned(),
                name: "Example".to_owned(),
                directory: Some("Example App".to_owned()),
                icon: Some(icon_path.display().to_string()),
            }],
        )
        .expect("target manifest should build");
        let expected_icon = format!(
            "$INSTALLPATH/bin/cargo-crapapp-shortcut-icon-{}.ico",
            std::process::id()
        );

        assert!(manifest.files.iter().any(|file| {
            file.source == icon_path.display().to_string()
                && file.destination == expected_icon.as_str()
        }));
        assert_eq!(
            manifest.shortcuts[0].icon.as_deref(),
            Some(expected_icon.as_str())
        );

        let _ = std::fs::remove_file(icon_path);
    }

    #[test]
    fn duplicate_shortcut_icons_only_add_one_payload() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-shortcut-icon-shared-{}.ico",
            std::process::id()
        ));
        std::fs::write(&icon_path, b"icon").expect("failed to write icon");

        let manifest = TargetManifest::new(
            "x86_64-pc-windows-gnu",
            &[String::from("example")],
            Some("$INSTALLPATH"),
            "bin",
            &[],
            &[
                ShortcutMapping {
                    binary: "example".to_owned(),
                    name: "Example 1".to_owned(),
                    directory: None,
                    icon: Some(icon_path.display().to_string()),
                },
                ShortcutMapping {
                    binary: "example".to_owned(),
                    name: "Example 2".to_owned(),
                    directory: None,
                    icon: Some(icon_path.display().to_string()),
                },
            ],
        )
        .expect("target manifest should build");

        let payload_count = manifest
            .files
            .iter()
            .filter(|file| file.source == icon_path.display().to_string())
            .count();
        assert_eq!(payload_count, 1);

        let _ = std::fs::remove_file(icon_path);
    }
}

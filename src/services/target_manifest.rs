use serde::Serialize;
use std::path::Path;

use anyhow::{Result, bail};

use crate::services::manifest_file::ShortcutMapping;
use crate::services::payload_file::{PayloadFile, resolve_destination};

#[derive(Debug, Serialize)]
pub struct TargetManifest {
    pub target: String,
    pub files: Vec<PayloadFile>,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Shortcut {
    pub target: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
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

        files.extend(extra_files.iter().cloned());
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

fn binary_file_name(target: &str, binary_name: &str) -> String {
    if target.contains("windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_owned()
    }
}

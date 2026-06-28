use anyhow::{Result, bail};
use serde::Serialize;

use crate::services::payload_file::{PayloadFile, join_payload_path, resolve_destination};

#[derive(Debug, Serialize)]
pub struct TargetManifest {
    pub target: String,
    pub files: Vec<PayloadFile>,
}

impl TargetManifest {
    pub fn new(
        target: &str,
        binary_names: &[String],
        install_path: Option<&str>,
        bin_dir: &str,
        extra_files: &[PayloadFile],
    ) -> Self {
        let mut files = binary_names
            .iter()
            .map(|binary| {
                let binary_file_name = binary_file_name(target, binary);

                PayloadFile::executable(
                    format!("target/{}/release/{}", target, binary_file_name),
                    resolve_destination(
                        install_path,
                        &join_payload_path(bin_dir, &binary_file_name),
                    ),
                )
            })
            .collect::<Vec<_>>();

        files.extend(extra_files.iter().cloned());

        Self {
            target: target.to_owned(),
            files,
        }
    }
}

fn binary_file_name(target: &str, binary_name: &str) -> String {
    if target.contains("windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_owned()
    }
}

pub fn validate_target_supported(target: &str) -> Result<()> {
    if target.ends_with("pc-windows-msvc") && !cfg!(target_os = "windows") {
        bail!("{target} can only be built on Windows");
    }

    if target.ends_with("apple-darwin") && !cfg!(target_os = "macos") {
        bail!("{target} can only be built on macOS");
    }

    Ok(())
}

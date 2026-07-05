use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::services::manifest_file::FileMapping;

#[derive(Clone, Debug, Serialize)]
pub struct PayloadFile {
    pub source: String,
    pub destination: String,
    pub executable: bool,
}

impl PayloadFile {
    pub fn executable(source: String, destination: String) -> Self {
        Self {
            source,
            destination,
            executable: true,
        }
    }

    pub fn data(source: String, destination: String) -> Self {
        Self {
            source,
            destination,
            executable: false,
        }
    }
}

pub fn resolve_destination(install_path: Option<&str>, destination: &str) -> String {
    match install_path {
        Some(install_path) if !destination.starts_with(install_path) => {
            join_payload_path(install_path, destination)
        }
        _ => destination.to_owned(),
    }
}

pub fn join_payload_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), child)
    }
}

pub fn payload_files(
    files: &[FileMapping],
    install_path: Option<&str>,
) -> Result<Vec<PayloadFile>> {
    let mut payload_files = Vec::new();

    for file in files {
        let source = Path::new(&file.source);

        if source.is_dir() {
            payload_files.extend(directory_payload_files(
                source,
                &file.destination,
                install_path,
            )?);
        } else if source.is_file() {
            payload_files.push(PayloadFile::data(
                file.source.clone(),
                resolve_destination(install_path, &file.destination),
            ));
        } else {
            bail!("payload source {} does not exist", source.display());
        }
    }

    Ok(payload_files)
}

fn directory_payload_files(
    source: &Path,
    destination: &str,
    install_path: Option<&str>,
) -> Result<Vec<PayloadFile>> {
    let mut files = Vec::new();
    collect_directory_payload_files(source, source, destination, install_path, &mut files)?;
    files.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(files)
}

fn collect_directory_payload_files(
    root: &Path,
    current: &Path,
    destination: &str,
    install_path: Option<&str>,
    files: &mut Vec<PayloadFile>,
) -> Result<()> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read payload directory {}", current.display()))?
    {
        entries.push(entry.with_context(|| {
            format!(
                "failed to read payload directory entry in {}",
                current.display()
            )
        })?);
    }

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            collect_directory_payload_files(root, &path, destination, install_path, files)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .with_context(|| format!("failed to calculate relative path for {}", path.display()))?;
        let destination = join_payload_path(destination, &path_to_payload_string(relative_path));

        files.push(PayloadFile::data(
            path.display().to_string(),
            resolve_destination(install_path, &destination),
        ));
    }

    Ok(())
}

fn path_to_payload_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

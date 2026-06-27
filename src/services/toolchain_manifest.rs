use anyhow::{Result, bail};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ToolchainManifest {
    pub toolchain: String,
    pub files: Vec<PayloadFile>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum BuildVariable {
    #[serde(rename = "INSTALLPATH")]
    InstallPath,
}

impl BuildVariable {
    pub fn name(self) -> &'static str {
        match self {
            BuildVariable::InstallPath => "INSTALLPATH",
        }
    }
}

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

impl ToolchainManifest {
    pub fn new(
        rust_target: &str,
        binary_names: &[String],
        install_path: Option<&str>,
        bin_dir: &str,
        extra_files: &[PayloadFile],
    ) -> Self {
        let mut files = binary_names
            .iter()
            .map(|binary| {
                let binary_file_name = binary_file_name(rust_target, binary);

                PayloadFile::executable(
                    format!("target/{}/release/{}", rust_target, binary_file_name),
                    resolve_destination(
                        install_path,
                        &join_payload_path(bin_dir, &binary_file_name),
                    ),
                )
            })
            .collect::<Vec<_>>();

        files.extend(extra_files.iter().cloned());

        Self {
            toolchain: rust_target.to_owned(),
            files,
        }
    }
}

pub fn variables_from_files(files: &[PayloadFile]) -> Result<Vec<BuildVariable>> {
    let mut variables = files
        .iter()
        .flat_map(|file| {
            [
                variables_from_value(&file.source),
                variables_from_value(&file.destination),
            ]
        })
        .flatten()
        .map(|name| BuildVariable::try_from(name.as_str()))
        .collect::<Result<Vec<_>>>()?;

    variables.sort();
    variables.dedup();
    Ok(variables)
}

pub fn variables_from_value(value: &str) -> Vec<String> {
    let mut variables = Vec::new();
    let mut chars = value.char_indices().peekable();

    while let Some((_, current)) = chars.next() {
        if current != '$' {
            continue;
        }

        let Some((start, first)) = chars.peek().copied() else {
            continue;
        };

        if !(first == '_' || first.is_ascii_alphabetic()) {
            continue;
        }

        let mut end = start + first.len_utf8();
        chars.next();

        while let Some((index, next)) = chars.peek().copied() {
            if !(next == '_' || next.is_ascii_alphanumeric()) {
                break;
            }

            end = index + next.len_utf8();
            chars.next();
        }

        variables.push(value[start..end].to_owned());
    }

    variables
}

impl TryFrom<&str> for BuildVariable {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "INSTALLPATH" => Ok(BuildVariable::InstallPath),
            unknown => bail!("unknown build variable ${unknown}"),
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

fn join_payload_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), child)
    }
}

fn binary_file_name(rust_target: &str, binary_name: &str) -> String {
    if rust_target.contains("windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_owned()
    }
}

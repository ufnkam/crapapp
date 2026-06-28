use anyhow::{Result, bail};
use serde::Serialize;

use crate::services::payload_file::PayloadFile;

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

impl TryFrom<&str> for BuildVariable {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "INSTALLPATH" => Ok(BuildVariable::InstallPath),
            unknown => bail!("unknown build variable ${unknown}"),
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

pub fn platform_variables(
    variable_sources: &[&str],
    files: &[PayloadFile],
) -> Result<Vec<BuildVariable>> {
    let mut variables = variable_sources
        .iter()
        .flat_map(|value| variables_from_value(value))
        .map(|name| BuildVariable::try_from(name.as_str()))
        .collect::<Result<Vec<_>>>()?;

    variables.extend(variables_from_files(files)?);
    variables.sort();
    variables.dedup();
    Ok(variables)
}

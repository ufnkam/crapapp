use std::str::FromStr;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::services::payload_file::PayloadFile;

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, strum::EnumString, strum::Display,
)]
pub enum BuildVariable {
    #[serde(rename = "INSTALLPATH")]
    #[strum(serialize = "INSTALLPATH")]
    InstallPath,
    #[serde(rename = "HOMEPATH")]
    #[strum(serialize = "HOMEPATH")]
    HomePath,
}

impl BuildVariable {
    fn is_runtime_provided(self) -> bool {
        matches!(self, Self::HomePath)
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
        .map(|name| {
            BuildVariable::from_str(&name)
                .with_context(|| format!("Variable {} is not supported", name))
        })
        .collect::<Result<Vec<BuildVariable>>>()?;

    variables.retain(|variable| !variable.is_runtime_provided());
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

pub fn get_platform_variables(
    variable_sources: &[&str],
    files: &[PayloadFile],
) -> Result<Vec<BuildVariable>> {
    let mut variables = variable_sources
        .iter()
        .flat_map(|value| variables_from_value(value))
        .map(|name| {
            BuildVariable::from_str(&name)
                .with_context(|| format!("Variable {} is not supported", &name))
        })
        .collect::<Result<Vec<BuildVariable>>>()?;

    variables.extend(variables_from_files(files)?);
    variables.retain(|variable| !variable.is_runtime_provided());
    variables.sort();
    variables.dedup();
    Ok(variables)
}

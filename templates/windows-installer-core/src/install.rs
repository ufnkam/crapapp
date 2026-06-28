use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{ADD_TO_PATH_VARIABLE, InstallerConfig, UNINSTALLER_EXE};
use crate::registry::{
    RegistryEntry, RegistryValue, registry_install_exists, write_registry_entries,
};

const PAYLOAD_KEY: u8 = 0xa5;

#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub install_root: PathBuf,
    pub uninstaller_path: PathBuf,
    pub existing: ExistingInstall,
    payload_paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
pub struct ExistingInstall {
    pub path_exists: bool,
    pub registry_exists: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct InstallReport {
    pub files: usize,
    pub estimated_size_kb: u32,
    pub path_updated: bool,
}

pub fn validate_variables(
    config: &InstallerConfig,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    for required in config.required_variables {
        if !values.contains_key(*required) {
            return Err(format!(
                "missing variable {required}. Pass it as --args {required}=<value>"
            ));
        }
    }

    for key in values.keys() {
        if !config.required_variables.contains(&key.as_str()) && key != ADD_TO_PATH_VARIABLE {
            return Err(format!("unknown variable {key}"));
        }
    }

    Ok(())
}

pub fn install_plan(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
) -> Result<InstallPlan, String> {
    let payload_paths = config
        .payload
        .iter()
        .map(|entry| PathBuf::from(resolve_variables(entry.destination, variables)))
        .collect::<Vec<_>>();
    let install_root = install_root(variables, &payload_paths)?;
    let existing = existing_install(config, &install_root);
    let uninstaller_path = install_root.join(UNINSTALLER_EXE);

    Ok(InstallPlan {
        install_root,
        uninstaller_path,
        existing,
        payload_paths,
    })
}

pub fn prune_install_root(install_root: &Path, uninstaller_path: &Path) -> Result<(), String> {
    if !install_root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(install_root)
        .map_err(|error| format!("failed to read {}: {error}", install_root.display()))?
    {
        let path = entry
            .map_err(|error| format!("failed to read {} entry: {error}", install_root.display()))?
            .path();

        if path == uninstaller_path {
            continue;
        }

        if path.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }
    }

    Ok(())
}

pub fn install(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
    plan: &InstallPlan,
) -> Result<InstallReport, String> {
    let mut installed_paths = Vec::new();

    for (entry, path) in config.payload.iter().zip(plan.payload_paths.iter()) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }

        fs::write(path, decode_payload(entry.bytes))
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

        #[cfg(unix)]
        if entry.executable {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path)
                .map_err(|error| format!("failed to read {} metadata: {error}", path.display()))?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)
                .map_err(|error| format!("failed to chmod {}: {error}", path.display()))?;
        }

        installed_paths.push(path.clone());
    }

    fs::create_dir_all(&plan.install_root)
        .map_err(|error| format!("failed to create {}: {error}", plan.install_root.display()))?;

    fs::write(&plan.uninstaller_path, config.uninstaller_bytes).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            plan.uninstaller_path.display()
        )
    })?;
    let estimated_size_kb = estimated_size_kb(&installed_paths, &plan.uninstaller_path)?;
    let path_updated = add_to_path_requested(variables)?;

    if path_updated {
        add_user_path_entries(config, variables)?;
    }

    write_registry_entries(registry_entries(
        config,
        variables,
        &plan.install_root,
        &plan.uninstaller_path,
        estimated_size_kb,
    ))?;

    Ok(InstallReport {
        files: config.payload.len(),
        estimated_size_kb,
        path_updated,
    })
}

fn decode_payload(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(|byte| byte ^ PAYLOAD_KEY).collect()
}

pub fn add_to_path_requested(values: &HashMap<String, String>) -> Result<bool, String> {
    values
        .get(ADD_TO_PATH_VARIABLE)
        .map(|value| parse_bool(value))
        .unwrap_or(Ok(true))
}

pub fn resolve_variables(value: &str, variables: &HashMap<String, String>) -> String {
    let mut resolved = value.to_owned();

    for (key, replacement) in variables {
        resolved = resolved.replace(&format!("${key}"), replacement);
    }

    resolved
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(format!(
            "invalid {ADD_TO_PATH_VARIABLE} value {value}, expected 1 or 0"
        )),
    }
}

fn install_root(
    variables: &HashMap<String, String>,
    installed_paths: &[PathBuf],
) -> Result<PathBuf, String> {
    if let Some(install_path) = variables.get("INSTALLPATH") {
        return Ok(PathBuf::from(install_path));
    }

    if let Some(parent) = installed_paths.first().and_then(|path| path.parent()) {
        return Ok(parent.to_path_buf());
    }

    env::current_dir().map_err(|error| format!("failed to find current directory: {error}"))
}

fn existing_install(config: &InstallerConfig, install_root: &Path) -> ExistingInstall {
    ExistingInstall {
        path_exists: install_root.exists(),
        registry_exists: registry_install_exists(config),
    }
}

fn estimated_size_kb(installed_paths: &[PathBuf], uninstaller_path: &Path) -> Result<u32, String> {
    let mut bytes = fs::metadata(uninstaller_path)
        .map_err(|error| {
            format!(
                "failed to read {} metadata: {error}",
                uninstaller_path.display()
            )
        })?
        .len();

    for path in installed_paths {
        bytes += fs::metadata(path)
            .map_err(|error| format!("failed to read {} metadata: {error}", path.display()))?
            .len();
    }

    Ok(bytes.div_ceil(1024).min(u32::MAX as u64) as u32)
}

fn registry_entries(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
    install_root: &Path,
    uninstaller_path: &Path,
    estimated_size_kb: u32,
) -> Vec<RegistryEntry> {
    let install_location = install_root.display().to_string();
    let uninstall_string = format!("\"{}\"", uninstaller_path.display());
    let key = crate::registry::uninstall_registry_key(config);

    let mut entries = vec![
        RegistryEntry {
            key: key.clone(),
            name: "DisplayName",
            value: RegistryValue::String(config.app_name.to_owned()),
        },
        RegistryEntry {
            key: key.clone(),
            name: "DisplayVersion",
            value: RegistryValue::String(config.app_version.to_owned()),
        },
        RegistryEntry {
            key: key.clone(),
            name: "InstallLocation",
            value: RegistryValue::String(install_location),
        },
        RegistryEntry {
            key: key.clone(),
            name: "UninstallString",
            value: RegistryValue::String(uninstall_string.clone()),
        },
        RegistryEntry {
            key: key.clone(),
            name: "QuietUninstallString",
            value: RegistryValue::String(uninstall_string),
        },
        RegistryEntry {
            key: key.clone(),
            name: "EstimatedSize",
            value: RegistryValue::U32(estimated_size_kb),
        },
    ];

    if let Some(publisher) = config.publisher {
        entries.push(RegistryEntry {
            key: key.clone(),
            name: "Publisher",
            value: RegistryValue::String(publisher.to_owned()),
        });
    }

    if let Some(display_icon) = config.app_display_icon {
        entries.push(RegistryEntry {
            key,
            name: "DisplayIcon",
            value: RegistryValue::String(resolve_variables(display_icon, variables)),
        });
    }

    entries
}

#[cfg(windows)]
fn add_user_path_entries(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let entries = config
        .path_entries
        .iter()
        .map(|entry| resolve_variables(entry, variables))
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Ok(());
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (environment, _) = hkcu
        .create_subkey("Environment")
        .map_err(|error| format!("failed to open user environment registry key: {error}"))?;
    let current_path = environment
        .get_value::<String, _>("Path")
        .unwrap_or_default();
    let mut path_parts = current_path
        .split(';')
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    for entry in entries {
        if !path_parts
            .iter()
            .any(|part| part.eq_ignore_ascii_case(&entry))
        {
            path_parts.push(entry);
        }
    }

    environment
        .set_value("Path", &path_parts.join(";"))
        .map_err(|error| format!("failed to update user Path: {error}"))
}

#[cfg(not(windows))]
fn add_user_path_entries(
    _config: &InstallerConfig,
    _variables: &HashMap<String, String>,
) -> Result<(), String> {
    Ok(())
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::InstallerConfig;
use crate::registry::remove_registry_key;

#[derive(Clone, Copy, Debug)]
pub struct UninstallOptions {
    pub keep_path: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct UninstallReport {
    pub files: usize,
    pub path_removed: bool,
}

pub fn uninstall(
    config: &InstallerConfig,
    options: UninstallOptions,
) -> Result<UninstallReport, String> {
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to find uninstaller path: {error}"))?;
    let install_root = current_exe
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "failed to find uninstaller directory".to_owned())?;

    let mut files = 0;
    for path in config
        .uninstall_entries
        .iter()
        .rev()
        .map(|entry| resolve_install_path(entry, &install_root))
    {
        if path == current_exe {
            continue;
        }

        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
            files += 1;
        }
    }

    remove_created_directories(config, &install_root);

    let path_removed = if options.keep_path {
        false
    } else {
        remove_user_path_entries(config, &install_root)
    };

    remove_registry_key(config);

    Ok(UninstallReport {
        files,
        path_removed,
    })
}

fn remove_created_directories(config: &InstallerConfig, install_root: &Path) {
    let mut directories = config
        .uninstall_entries
        .iter()
        .filter_map(|entry| {
            resolve_install_path(entry, install_root)
                .parent()
                .map(PathBuf::from)
        })
        .collect::<Vec<_>>();
    directories.sort();
    directories.dedup();
    directories.sort_by_key(|path| path.components().count());

    for directory in directories.iter().rev() {
        let _ = fs::remove_dir(directory);
    }
}

fn resolve_install_path(value: &str, install_root: &Path) -> PathBuf {
    let install_root = install_root.display().to_string();

    if value.contains("$INSTALLPATH") {
        return PathBuf::from(value.replace("$INSTALLPATH", &install_root));
    }

    let path = PathBuf::from(value);

    if path.is_absolute() {
        path
    } else {
        PathBuf::from(install_root).join(path)
    }
}

#[cfg(windows)]
fn remove_user_path_entries(config: &InstallerConfig, install_root: &Path) -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let entries = config
        .path_entries
        .iter()
        .map(|entry| {
            resolve_install_path(entry, install_root)
                .display()
                .to_string()
        })
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return false;
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(environment) = hkcu.open_subkey_with_flags(
        "Environment",
        winreg::enums::KEY_READ | winreg::enums::KEY_WRITE,
    ) else {
        return false;
    };
    let current_path = environment
        .get_value::<String, _>("Path")
        .unwrap_or_default();
    let path_parts = current_path
        .split(';')
        .filter(|part| {
            !part.is_empty() && !entries.iter().any(|entry| part.eq_ignore_ascii_case(entry))
        })
        .collect::<Vec<_>>();

    environment.set_value("Path", &path_parts.join(";")).is_ok()
}

#[cfg(not(windows))]
fn remove_user_path_entries(_config: &InstallerConfig, _install_root: &Path) -> bool {
    false
}

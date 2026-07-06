use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use crate::windows_installer::config::{AssociatedFileKind, InstallerConfig};
use crate::windows_installer::install::uninstall_entries;

pub fn remove_created_directories(config: &InstallerConfig, install_root: &Path) {
    let mut directories = uninstall_entries(config)
        .into_iter()
        .filter_map(|entry| {
            resolve_install_path(entry.into(), install_root)
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

pub fn remove_associated_files(
    config: &InstallerConfig,
    install_root: &Path,
) -> Result<(), String> {
    for entry in config.associated_files.iter().rev() {
        let path = resolve_install_path(Cow::from(entry.path.as_str()), install_root);

        if !path.exists() {
            continue;
        }

        match entry.kind {
            AssociatedFileKind::File => fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?,
            AssociatedFileKind::Directory => fs::remove_dir_all(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?,
        }
    }

    Ok(())
}

pub fn resolve_install_path(value: Cow<'_, str>, install_root: &Path) -> PathBuf {
    let install_root = install_root.display().to_string();
    let value = value.into_owned();
    let value = value.replace("$INSTALLPATH", &install_root);
    let value = home_path()
        .map(|home_path| value.replace("$HOMEPATH", &home_path.display().to_string()))
        .unwrap_or(value);
    let path = Path::new(&value).components().collect::<PathBuf>();

    if path.is_absolute() {
        path
    } else {
        PathBuf::from(install_root).join(path)
    }
}

fn home_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

#[cfg(windows)]
pub fn remove_user_path_entries(config: &InstallerConfig, install_root: &Path) -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let entries = crate::windows_installer::install::path_entries(config)
        .into_iter()
        .map(|entry| {
            resolve_install_path(entry.into(), install_root)
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
pub fn remove_user_path_entries(_config: &InstallerConfig, _install_root: &Path) -> bool {
    false
}

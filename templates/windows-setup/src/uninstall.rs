#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const APP_NAME: &str = __CRAPAPP_APP_NAME__;
const UNINSTALL_ENTRIES: &[&str] = &[__CRAPAPP_UNINSTALL_ENTRIES__];
const PATH_ENTRIES: &[&str] = &[__CRAPAPP_PATH_ENTRIES__];

fn main() {
    if let Err(error) = uninstall() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn uninstall() -> Result<(), String> {
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to find uninstaller path: {error}"))?;
    let install_root = current_exe
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "failed to find uninstaller directory".to_owned())?;

    for path in UNINSTALL_ENTRIES
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
        }
    }

    remove_user_path_entries(&install_root);
    unregister_uninstaller();

    let _ = fs::remove_file(&current_exe);

    println!("uninstalled {APP_NAME}");
    Ok(())
}

fn resolve_install_path(value: &str, install_root: &PathBuf) -> PathBuf {
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

fn remove_user_path_entries(install_root: &PathBuf) {
    let entries = PATH_ENTRIES
        .iter()
        .map(|entry| resolve_install_path(entry, install_root).display().to_string())
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return;
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(environment) = hkcu.open_subkey_with_flags(
        "Environment",
        winreg::enums::KEY_READ | winreg::enums::KEY_WRITE,
    ) else {
        return;
    };
    let current_path = environment.get_value::<String, _>("Path").unwrap_or_default();
    let path_parts = current_path
        .split(';')
        .filter(|part| {
            !part.is_empty()
                && !entries
                    .iter()
                    .any(|entry| part.eq_ignore_ascii_case(entry))
        })
        .collect::<Vec<_>>();

    let _ = environment.set_value("Path", &path_parts.join(";"));
}

fn unregister_uninstaller() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(registry_key());
}

fn registry_key() -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{APP_NAME}")
}

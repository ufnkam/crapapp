use std::fs;
use std::path::{Path, PathBuf};

use crate::windows_installer::config::InstallerConfig;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
#[cfg(windows)]
use windows::core::{Interface, PCWSTR};

#[cfg(windows)]
pub fn create_start_menu_shortcuts(
    config: &InstallerConfig,
    variables: &std::collections::HashMap<String, String>,
    install_root: &Path,
) -> Result<(), String> {
    use std::borrow::Cow;

    use crate::windows_installer::install::resolve_variables;
    use crate::windows_installer::resolve_install_path;

    if config.shortcuts.is_empty() {
        return Ok(());
    }

    let programs_dir = programs_dir()?;
    let _com = ComApartment::new()?;

    for shortcut in &config.shortcuts {
        let target = resolve_install_path(
            Cow::from(resolve_variables(&shortcut.target, variables)),
            install_root,
        );
        let shortcut_dir = shortcut
            .directory
            .as_deref()
            .map(shortcut_name)
            .transpose()?
            .map(|directory| programs_dir.join(directory))
            .unwrap_or_else(|| programs_dir.clone());
        let shortcut_path = shortcut_dir.join(format!("{}.lnk", shortcut_name(&shortcut.name)?));
        let working_dir = target.parent().unwrap_or(install_root);

        fs::create_dir_all(&shortcut_dir)
            .map_err(|error| format!("failed to create {}: {error}", shortcut_dir.display()))?;

        let target = wide(target.as_os_str());
        let working_dir = wide(working_dir.as_os_str());
        let shortcut_path = wide(shortcut_path.as_os_str());

        unsafe {
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("failed to create ShellLink COM object: {error}"))?;
            shell_link
                .SetPath(PCWSTR(target.as_ptr()))
                .map_err(|error| format!("failed to set shortcut target: {error}"))?;
            shell_link
                .SetWorkingDirectory(PCWSTR(working_dir.as_ptr()))
                .map_err(|error| format!("failed to set shortcut working directory: {error}"))?;
            shell_link
                .SetIconLocation(PCWSTR(target.as_ptr()), 0)
                .map_err(|error| format!("failed to set shortcut icon: {error}"))?;

            let persist_file: IPersistFile = shell_link
                .cast()
                .map_err(|error| format!("failed to open shortcut persistence API: {error}"))?;
            persist_file
                .Save(PCWSTR(shortcut_path.as_ptr()), true)
                .map_err(|error| format!("failed to save start menu shortcut: {error}"))?;
        }
    }

    Ok(())
}

#[cfg(windows)]
struct ComApartment;

#[cfg(windows)]
impl ComApartment {
    fn new() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| format!("failed to initialize COM: {error}"))?;
        }

        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
pub fn create_start_menu_shortcuts(
    _config: &InstallerConfig,
    _variables: &std::collections::HashMap<String, String>,
    _install_root: &Path,
) -> Result<(), String> {
    Ok(())
}

pub fn remove_start_menu_shortcuts(config: &InstallerConfig) {
    if config.shortcuts.is_empty() {
        return;
    }

    let Ok(programs_dir) = programs_dir() else {
        return;
    };

    for shortcut in &config.shortcuts {
        let Ok(name) = shortcut_name(&shortcut.name) else {
            continue;
        };
        let shortcut_dir = shortcut
            .directory
            .as_deref()
            .and_then(|directory| shortcut_name(directory).ok())
            .map(|directory| programs_dir.join(directory))
            .unwrap_or_else(|| programs_dir.clone());

        let _ = fs::remove_file(shortcut_dir.join(format!("{name}.lnk")));
        let _ = fs::remove_dir(shortcut_dir);
    }
}

fn programs_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| "failed to find APPDATA for Start Menu shortcuts".to_owned())?;

    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs"))
}

fn shortcut_name(value: &str) -> Result<String, String> {
    let name = value
        .trim()
        .chars()
        .map(|character| match character {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            character => character,
        })
        .collect::<String>();

    if name.is_empty() {
        Err("shortcut name cannot be empty".to_owned())
    } else {
        Ok(name)
    }
}

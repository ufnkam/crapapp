use std::path::Path;

use crate::windows_installer::config::InstallerConfig;

#[cfg_attr(not(windows), allow(dead_code))]
pub struct RegistryEntry {
    pub key: String,
    pub name: &'static str,
    pub value: RegistryValue,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub enum RegistryValue {
    String(String),
    U32(u32),
}

#[cfg(windows)]
pub fn registry_install_exists(config: &InstallerConfig, install_root: &Path) -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(uninstall_registry_key(config, install_root))
        .is_ok()
}

#[cfg(not(windows))]
pub fn registry_install_exists(_config: &InstallerConfig, _install_root: &Path) -> bool {
    false
}

#[cfg(windows)]
pub fn write_registry_entries(entries: Vec<RegistryEntry>) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for entry in entries {
        let (key, _) = hkcu.create_subkey(&entry.key).map_err(|error| {
            format!("failed to create user registry key {}: {error}", entry.key)
        })?;

        match entry.value {
            RegistryValue::String(value) => key
                .set_value(entry.name, &value)
                .map_err(|error| format!("failed to write {}: {error}", entry.name))?,
            RegistryValue::U32(value) => key
                .set_value(entry.name, &value)
                .map_err(|error| format!("failed to write {}: {error}", entry.name))?,
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn write_registry_entries(_entries: Vec<RegistryEntry>) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn remove_registry_key(config: &InstallerConfig, install_root: &Path) {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(uninstall_registry_key(config, install_root));
}

#[cfg(not(windows))]
pub fn remove_registry_key(_config: &InstallerConfig, _install_root: &Path) {}

pub fn uninstall_registry_key(config: &InstallerConfig, install_root: &Path) -> String {
    format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}-{}",
        config.app_name,
        install_identity(install_root)
    )
}

fn install_identity(install_root: &Path) -> String {
    install_root
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .flat_map(|component| component.chars())
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

use crate::config::InstallerConfig;

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct RegistryEntry {
    pub key: String,
    pub name: &'static str,
    pub value: RegistryValue,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum RegistryValue {
    String(String),
    U32(u32),
}

#[cfg(windows)]
pub(crate) fn registry_install_exists(config: &InstallerConfig) -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(uninstall_registry_key(config)).is_ok()
}

#[cfg(not(windows))]
pub(crate) fn registry_install_exists(_config: &InstallerConfig) -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn write_registry_entries(entries: Vec<RegistryEntry>) -> Result<(), String> {
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
pub(crate) fn write_registry_entries(_entries: Vec<RegistryEntry>) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub(crate) fn remove_registry_key(config: &InstallerConfig) {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(uninstall_registry_key(config));
}

#[cfg(not(windows))]
pub(crate) fn remove_registry_key(_config: &InstallerConfig) {}

pub(crate) fn uninstall_registry_key(config: &InstallerConfig) -> String {
    format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        config.app_name
    )
}

#[cfg(all(feature = "windows-cli", feature = "windows-gui"))]
compile_error!("features `windows-cli` and `windows-gui` cannot be enabled together");

#[cfg(not(any(feature = "windows-cli", feature = "windows-gui")))]
compile_error!("enable either feature `windows-cli` or `windows-gui`");

#[cfg(feature = "windows-cli")]
pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    crate::windows_installer::cli::entrypoints::uninstaller::run(config, payload, uninstaller)
}

#[cfg(feature = "windows-gui")]
pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    crate::windows_installer::gui::uninstaller::run(config, payload, uninstaller)
}

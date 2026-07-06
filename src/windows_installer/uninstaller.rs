#[cfg(all(feature = "cli", feature = "gui"))]
compile_error!("features `cli` and `gui` cannot be enabled together");

#[cfg(not(any(feature = "cli", feature = "gui")))]
compile_error!("enable either feature `cli` or `gui`");

#[cfg(feature = "cli")]
pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    crate::windows_installer::cli::entrypoints::uninstaller::run(config, payload, uninstaller)
}

#[cfg(feature = "gui")]
pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    crate::windows_installer::gui::uninstaller::run(config, payload, uninstaller)
}

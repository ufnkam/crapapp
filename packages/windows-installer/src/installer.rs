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
    crate::cli::entrypoints::installer::run(config, payload, uninstaller)
}

#[cfg(feature = "gui")]
pub fn run(
    _config: &'static [u8],
    _payload: &'static [u8],
    _uninstaller: &'static [u8],
) -> Result<(), String> {
    crate::gui::installer::run().map_err(|error| error.to_string())
}

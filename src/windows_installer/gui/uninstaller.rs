use super::app::{EntryPoint, UiConfig};

pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    let config = UiConfig::from_embedded(EntryPoint::Uninstaller, config, payload, uninstaller)?;
    super::app::run(config).map_err(|error| error.to_string())
}

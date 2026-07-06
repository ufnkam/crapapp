use clap::Parser;

use crate::windows_installer::InstallerConfig;

#[derive(Debug, Parser)]
#[command(name = "uninstall")]
#[command(about = "Uninstall the packaged application")]
struct Cli {}

pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    let _args = Cli::parse();
    let config = InstallerConfig::new(config, payload, uninstaller)?;
    crate::windows_installer::cli::commands::uninstall(&config)
}

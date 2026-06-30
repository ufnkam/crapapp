use std::process;

use clap::Parser;
use windows_installer_core::{InstallerConfig, cli};

const SETUP_CONFIG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/setup-config.json"));
const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));
const UNINSTALLER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uninstall.exe"));

#[derive(Debug, Parser)]
#[command(name = "uninstall")]
#[command(about = "Uninstall the packaged application")]
struct Cli {}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let _args = Cli::parse();
    let config = installer_config()?;
    cli::uninstall(&config)
}

fn installer_config() -> Result<InstallerConfig, String> {
    windows_installer_core::installer_config(SETUP_CONFIG, PAYLOAD, UNINSTALLER)
}

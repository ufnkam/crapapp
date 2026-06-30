use std::collections::HashMap;
use std::process;

use clap::Parser;
use windows_installer_core::{cli, InstallerConfig};

const SETUP_CONFIG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/setup-config.json"));
const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));
const UNINSTALLER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uninstall.exe"));

#[derive(Debug, Parser)]
#[command(name = "setup")]
#[command(about = "Install the packaged application")]
struct Cli {
    /// Installer variable in KEY=value form. Repeat for each required variable.
    #[arg(long = "args", value_parser = parse_variable)]
    args: Vec<(String, String)>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = installer_config()?;
    let variables = parse_args();
    cli::install(&config, &variables)
}

fn installer_config() -> Result<InstallerConfig, String> {
    windows_installer_core::installer_config(SETUP_CONFIG, PAYLOAD, UNINSTALLER)
}

fn parse_args() -> HashMap<String, String> {
    Cli::parse().args.into_iter().collect()
}

fn parse_variable(value: &str) -> Result<(String, String), String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| format!("invalid --args value {value}, expected KEY=value"))?;

    Ok((key.to_owned(), value.to_owned()))
}


#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process;

use clap::Parser;
use crapapp_windows_installer_core::{UninstallOptions, cli};

mod generated;

#[derive(Debug, Parser)]
#[command(name = "uninstall")]
#[command(about = "Uninstall the packaged application")]
struct Cli {
    /// Keep PATH entries even if the installer added them.
    #[arg(long)]
    keep_path: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Cli::parse();
    cli::uninstall(
        &generated::CONFIG,
        UninstallOptions {
            keep_path: args.keep_path,
        },
    )
}

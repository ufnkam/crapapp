use std::env;
use std::ffi::OsString;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::services::{
    BuildManifest, BuildManifestFormatter, Builder, CrapManifest, MANIFEST_PATH,
};

#[derive(Debug, Parser)]
#[command(name = "crapapp")]
#[command(about = "Build installers and self-contained app bundles from CRAP.toml")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read and validate a CRAP.toml manifest.
    Inspect {
        /// Output format.
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
        output: OutputFormat,
    },
    /// Build configured cargo packages for configured platform targets.
    Build,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse_from(cargo_args());

    match cli.command {
        Command::Inspect { output } => {
            let manifest = CrapManifest::load(MANIFEST_PATH)?;
            let build_manifest = BuildManifest::from_crap_manifest(&manifest)?;
            let output = BuildManifestFormatter::from(output);

            print!("{}", build_manifest.display(output)?);
        }
        Command::Build => {
            let manifest = CrapManifest::load(MANIFEST_PATH)?;
            let build_manifest = BuildManifest::from_crap_manifest(&manifest)?;

            Builder::new(&build_manifest).build()?;
        }
    }

    Ok(())
}

impl From<OutputFormat> for BuildManifestFormatter {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Text => BuildManifestFormatter::Text,
            OutputFormat::Json => BuildManifestFormatter::Json,
        }
    }
}

fn cargo_args() -> Vec<OsString> {
    let mut args = env::args_os();
    let mut normalized = Vec::new();

    if let Some(bin) = args.next() {
        normalized.push(bin);
    }

    match args.next() {
        Some(arg) if arg == "crapapp" => {}
        Some(arg) => normalized.push(arg),
        None => {}
    }

    normalized.extend(args);
    normalized
}

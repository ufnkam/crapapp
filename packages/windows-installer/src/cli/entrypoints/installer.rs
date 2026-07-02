use clap::Parser;

use crate::InstallerConfig;

#[derive(Debug, Parser)]
#[command(name = "setup")]
#[command(about = "Install the packaged application")]
struct Cli {
    /// Installer variable in KEY=value form. Repeat for each required variable.
    #[arg(long = "args", value_parser = parse_variable)]
    args: Vec<(String, String)>,
}

pub fn run(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<(), String> {
    let config = InstallerConfig::new(config, payload, uninstaller)?;
    let variables = Cli::parse().args.into_iter().collect();
    crate::cli::commands::install(&config, &variables)
}

fn parse_variable(value: &str) -> Result<(String, String), String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| format!("invalid --args value {value}, expected KEY=value"))?;

    Ok((key.to_owned(), value.to_owned()))
}

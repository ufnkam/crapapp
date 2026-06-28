use std::collections::HashMap;
use std::io::{self, Write};
use std::process;

use clap::Parser;
use crapapp_windows_installer_core::cli;

mod generated;

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
    let variables = parse_args();
    cli::install(&generated::CONFIG, &variables, confirm)
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

fn confirm(question: &str) -> Result<bool, String> {
    print!("{question} [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush stdout: {error}"))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("failed to read answer: {error}"))?;

    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
}

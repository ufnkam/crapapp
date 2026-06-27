use anyhow::Result;

mod cli;
mod services;

use crate::cli::run_cli;

fn main() -> Result<()> {
    run_cli()
}

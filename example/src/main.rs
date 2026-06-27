use clap::{Parser};

#[derive(Debug, Parser)]
#[command(name = "example")]
#[command(about = "Tiny example CLI for cargo-crapapp")]
struct Cli {}


fn main() {
    let _cli = Cli::parse();

    println!("Hello, world!");
}

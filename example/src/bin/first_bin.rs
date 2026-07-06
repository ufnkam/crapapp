use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "example")]
#[command(about = "Tiny example CLI for cargo-crapapp")]
struct Cli {}

fn main() {
    println!("Hello and burn this world!");

    #[cfg(feature = "some_feature")]
    {
        println!("I'm using some_feature!")
    }

    #[cfg(not(feature = "some_feature"))]
    {
        println!("I'm not using some_feature!")
    }

    let _cli = Cli::parse();
}

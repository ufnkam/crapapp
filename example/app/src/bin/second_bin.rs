use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Parser)]
#[command(name = "second_bin", version, about = "Example diagnostics CLI")]
struct Cli {
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    #[arg(long)]
    include_paths: bool,
}

fn main() {
    let cli = Cli::parse();

    match cli.format {
        Format::Text => {
            println!("second_bin diagnostics");
            println!("package: example");
            println!("include_paths: {}", cli.include_paths);
            if cli.include_paths {
                println!("app: /Applications/Example App.app");
                println!("bin: /usr/local/bin/second_bin");
            }
        }
        Format::Json => {
            println!(
                r#"{{"package":"example","include_paths":{},"binary":"second_bin"}}"#,
                cli.include_paths
            );
        }
    }
}

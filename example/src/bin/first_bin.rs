use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "first_bin", version, about = "Example packaged CLI utility")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a greeting a configurable number of times.
    Greet {
        #[arg(long, default_value = "developer")]
        name: String,
        #[arg(long, default_value_t = 1)]
        times: u8,
    },
    /// Print compile-time feature information.
    Features,
}

fn main() {
    match Cli::parse().command {
        Command::Greet { name, times } => {
            for _ in 0..times {
                println!("Hello, {name}. first_bin is reachable from the package.");
            }
        }
        Command::Features => {
            #[cfg(feature = "some_feature")]
            println!("some_feature is enabled");

            #[cfg(not(feature = "some_feature"))]
            println!("some_feature is disabled");
        }
    }
}

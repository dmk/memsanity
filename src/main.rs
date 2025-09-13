mod config;
mod core;
mod drivers;
mod engine;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "memsanity",
    version,
    about = "Sanity/functional tester for in-memory stores"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a YAML suite
    Run {
        /// Path to a YAML suite file
        #[arg(value_name = "SUITE")]
        suite: String,

        /// Dry run only: parse and print the plan; do not execute
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Run { suite, dry_run } => {
            let suite = config::load_suite_from_path(&suite)?;
            if dry_run {
                println!("Loaded suite: {}", suite.name);
                engine::print_plan(&suite);
                return Ok(());
            }
            engine::execute_suite(&suite).await
        }
    }
}

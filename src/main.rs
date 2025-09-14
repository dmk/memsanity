mod config;
mod core;
mod drivers;
mod engine;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::path::Path;
use tracing::info;

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

        /// Write all logs as JSON to file (in addition to console)
        #[arg(short = 'o', long = "output", value_name = "FILE")]
        output: Option<String>,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Parse CLI first so we can configure logging based on flags
    let cli = Cli::parse();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Always log to console (human-readable)
    let stdout_layer = tracing_subscriber::fmt::layer();

    // Optionally, also log JSON to a file specified by -o/--output
    let mut _non_blocking_guard = None;
    let json_layer_opt = match &cli.command {
        Commands::Run { output: Some(output_path), .. } => {
            let path = Path::new(output_path);
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("memsanity.json");
            let file_appender = tracing_appender::rolling::never(parent, filename);
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            _non_blocking_guard = Some(guard);
            Some(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(non_blocking),
            )
        }
        _ => None,
    };

    if let Some(json_layer) = json_layer_opt {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(json_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .init();
    }

    match cli.command {
        Commands::Run { suite, dry_run, .. } => {
            let suite = config::load_suite_from_path(&suite)?;
            if dry_run {
                info!(suite = %suite.name, "dry_run_loaded");
                engine::print_plan(&suite);
                return Ok(());
            }
            engine::execute_suite(&suite).await
        }
    }
}

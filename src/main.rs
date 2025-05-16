use std::env;

use clap::Parser;

use cli::{Args, Commands};
use tracing::debug;

use structs::config::Config;

mod attacker;
mod cli;
mod database;
mod display;
mod structs;
mod submitter;
mod utils;

/// Sets `RUST_LOG` environment variable to `info` if it's not already set,
/// and initializes `pretty_env_logger`.
///
/// Also overrides the default panic handler to log the panic message and exit with (1).
fn init() {
    if env::var("RUST_LOG").unwrap_or_default().is_empty() {
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    // Initialize logging
    // logger::init_logging();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    init();

    // Load config from sources
    let config = Config::from_sources(&args)?;
    debug!("Loaded config: {:#?}", config);

    match &args.command {
        // TODO: add error handling to `attacker`
        Commands::Attack(_) => {
            attacker::tui::ui(args, &config).await;
            Ok(())
        }
        Commands::Display => display::print_flags(&config).map_err(|e| e.to_string().into()),
    }
}

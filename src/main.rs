use std::env;

use clap::Parser;

use cli::{Args, Commands};
use log::debug;

use structs::config::Config;

mod attacker;
mod cli;
mod database;
mod structs;
mod submitter;

/// Sets `RUST_LOG` environment variable to `info` if it's not already set,
/// and initializes `pretty_env_logger`.
///
/// Also overrides the default panic handler to log the panic message and exit with (1).
unsafe fn init() {
    if env::var("RUST_LOG").unwrap_or_default().is_empty() {
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    // Initialize logging
    pretty_env_logger::init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    unsafe {
        init();
    }

    // Load config from sources
    let config = Config::from_sources(&args)?;
    debug!("Loaded config: {:#?}", config);

    match &args.command {
        // TODO: add error handling to `attacker`
        Commands::Attack(_) => Ok(attacker::attack(&config).await),
    }
}

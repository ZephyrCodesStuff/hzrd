use std::env;

use clap::Parser;
use cli::{Args, Commands};
use config::Config;
use log::{error, info, warn};
use regex::Regex;

mod cli;
mod commands;
mod config;
mod subnet;

/// Handle panics by logging with `error!` and exiting with (1)
fn handle_panic(info: &std::panic::PanicHookInfo) {
    error!("Panic occurred: {}", info);
    std::process::exit(1);
}

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

    // Override the default panic handler
    std::panic::set_hook(Box::new(handle_panic));
}

fn main() {
    let args = Args::parse();
    unsafe {
        init();
    }

    let config = Config::init();

    match args.command {
        Commands::Run { script, subnet } => {
            let subnet = subnet.unwrap_or_else(|| {
                config.subnet.expect(
                    "Subnet is required (either as `--subnet` in the CLI, or in the config file.",
                )
            });

            let mut flags: Vec<String> = vec![];

            for host in subnet.0.hosts() {
                let mut captured = commands::run(script.clone(), host);

                // Make sure all flags pass the regex
                let regex = Regex::new(&config.flag_regex).unwrap();
                captured.retain(|flag| regex.is_match(flag));

                if captured.is_empty() {
                    warn!("The exploit did not work on {host}.");
                    continue;
                }

                info!("Flag captured on {host}!");
                flags.append(&mut captured);
            }

            if !flags.is_empty() {
                info!(
                    "Your exploit captured the following flags: {}",
                    flags.join(", ")
                );
            }
        }
    }
}

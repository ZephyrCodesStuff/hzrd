use std::env;

use clap::Parser;
use cli::{Args, Commands};
use config::Config;
use log::error;

mod cli;
mod config;
mod runner;
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
        Commands::Run {
            script,
            subnet,
            r#loop,
        } => {
            runner::run(config.clone(), script, subnet, r#loop);
        }
    }
}

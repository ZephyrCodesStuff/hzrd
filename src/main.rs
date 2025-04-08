use std::env;

use clap::Parser;
use cli::{Args, Commands};
use config::Config;
use log::error;

mod cli;
mod config;
mod runner;
mod submitter;
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

    // Get config from CLI-given file, or default `./hzrd.toml`
    let config = Config::new(args.config);

    match args.command {
        Commands::Run {
            script,
            subnet,
            hosts,
            r#loop,
            submit,
        } => {
            runner::run(config.clone(), script, subnet, hosts, r#loop, submit);
        }
    }
}

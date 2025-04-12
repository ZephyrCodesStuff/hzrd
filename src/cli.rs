use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,

    /// Override the default configuration.
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Run an exploit on a remote
    Attack(AttackerArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct AttackerArgs {
    /// Override the exploits to run.
    pub exploit: Option<PathBuf>,

    /// Override the hosts to attack.
    #[arg(long)]
    pub hosts: Option<Vec<String>>,

    /// If given, run the exploits every `x` seconds.
    ///
    /// Defaults to the config's value, or `None` if not specified.
    #[arg(long)]
    pub r#loop: Option<u64>,

    /// If active, submit the flags to the configured host.
    ///
    /// Defaults to the config's value, or `true` if not specified.
    #[arg(long, default_value_t = true)]
    pub submit: bool,
}

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::subnet::Subnet;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to the configuration file to use (defaults to: `./hzrd.toml`)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run an exploit on a remote
    Run {
        /// Python script containing the exploit to run.
        ///
        /// If this is a folder, all of the scripts inside will be run.
        ///
        /// If not provided, will fallback to the config's exploit directory.
        script: Option<PathBuf>,

        /// Override the remote subnet to attack.
        #[arg(short, long)]
        subnet: Option<Subnet>,

        /// Override the hosts to attack.
        #[arg(short, long, value_delimiter = ',')]
        hosts: Option<Vec<String>>,

        /// Run the exploits every `x` seconds
        #[arg(short, long)]
        r#loop: Option<u64>,

        /// If active, submit the flags to the configured host
        #[arg(long, default_value_t = false)]
        submit: bool,
    },
}

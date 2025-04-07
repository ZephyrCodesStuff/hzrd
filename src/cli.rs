use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::subnet::Subnet;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
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

        /// Run the exploits every `x` seconds
        #[arg(short, long)]
        r#loop: Option<u64>,
    },
}

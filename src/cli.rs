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
        /// Python script containing the exploit to run
        script: PathBuf,

        /// Override the remote subnet to attack.
        #[arg(short, long)]
        subnet: Option<Subnet>,
    },
}

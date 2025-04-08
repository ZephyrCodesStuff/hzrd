use std::{net::Ipv4Addr, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::subnet::Subnet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Subnet mask of remotes to attack
    pub subnet: Option<Subnet>,

    /// Hosts to attack
    pub hosts: Option<Vec<String>>,

    /// Regex for the flags to match against.
    ///
    /// If not provided, anything returned from `exploit()` will be
    /// considered a valid flag.
    pub flag_regex: Option<String>,

    /// Directory containing all of the exploits to run
    pub exploit_dir: Option<PathBuf>,

    /// Configuration for submitting flags to a specified TCP remote.
    pub submit: Option<SubmitConfig>,

    /// Display useful hints while running the program
    pub hints: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            subnet: None,
            hosts: None,
            flag_regex: None,
            exploit_dir: None,
            submit: Some(SubmitConfig::default()),
            hints: true,
        }
    }
}

impl Config {
    /// Initializes from the given file
    pub fn new(path: Option<PathBuf>) -> Self {
        let path = path.unwrap_or(PathBuf::from("hzrd.toml"));

        // Find config, or return default
        let Ok(file) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        // If found config, parse it
        toml::from_str(&file).expect("Failed to parse config")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitConfig {
    /// IP address of the server to submit flags to.
    pub host: Ipv4Addr,

    /// Port of the server to submit flags to.
    pub port: u16,

    /// Team token assigned, to identify with the remote.
    pub token: String,
}

impl Default for SubmitConfig {
    fn default() -> Self {
        Self {
            host: Ipv4Addr::new(127, 0, 0, 1),
            port: 8080,
            token: String::from("hzrd"),
        }
    }
}

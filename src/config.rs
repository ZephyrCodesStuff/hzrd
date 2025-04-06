use serde::{Deserialize, Serialize};

use crate::subnet::Subnet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Subnet mask of remotes to attack
    pub subnet: Option<Subnet>,

    /// Regex for the flags to match against
    pub flag_regex: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            subnet: None,
            flag_regex: String::new(),
        }
    }
}

impl Config {
    /// Initializes from `hzrd.toml`
    pub fn init() -> Self {
        // Find config, or return default
        let Ok(file) = std::fs::read_to_string("hzrd.toml") else {
            return Self::default();
        };

        // If found config, parse it
        toml::from_str(&file).expect("Failed to parse config")
    }
}

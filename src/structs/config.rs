use anyhow::Result;
use config::{Config as RawConfig, Environment, File};
use serde::{Deserialize, Serialize};
use url::Url;

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use crate::cli::{Args, Commands};

use super::errors::ConfigError;
use super::team::Team;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub attacker: AttackerConfig,
    pub submitter: Option<SubmitterConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttackerConfig {
    /// Path to a folder containing scripts, or to an individual script.
    ///
    /// Defaults to `${pwd}/exploits` if not specified.
    pub exploit: PathBuf,

    /// Flag regex to look for, in the scripts' output.
    pub flag: String,

    /// Teams to attack during the competition.
    pub teams: HashMap<String, Team>,

    /// Configure how the attacker should loop its attacks.
    ///
    /// Defaults to `None` (the attacker will only run them once.)
    pub r#loop: Option<AttackerLoopConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttackerLoopConfig {
    /// After how many **seconds** to loop the exploits.
    pub every: u64,

    /// If provided, will wait to make time-based detection more difficult.
    ///
    /// Defaults to `None` (0 seconds).
    pub random: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmitterConfig {
    /// Chosen configuration type
    pub r#type: String,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Configurations for submission system (TCP, HTTP, etc.).
    pub config: SubmissionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// Database file path
    pub file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmissionConfig {
    /// TCP configuration if selected
    #[serde(default)]
    pub tcp: Option<SubmitterTCPConfig>,

    /// HTTP configuration if selected
    #[serde(default)]
    pub http: Option<SubmitterHTTPConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmitterTCPConfig {
    /// IP address of the submitter system.
    pub host: Ipv4Addr,

    /// Port on which the submission system is listening.
    pub port: u16,

    /// Unique token to authenticate your team during the competition.
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmitterHTTPConfig {
    /// URL of the submitter system.
    pub url: Url,

    /// Unique token to authenticate your team during the competition.
    pub token: String,
}

impl Config {
    pub fn from_sources(cli: &Args) -> Result<Self> {
        let mut builder = RawConfig::builder();

        // 1. CLI-specified config file
        if let Some(ref path) = cli.config {
            builder = builder.add_source(File::from(path.clone()));
        }
        // 2. Local ./hzrd.toml
        else if Path::new("hzrd.toml").exists() {
            builder = builder.add_source(File::with_name("hzrd"));
        }
        // 3. XDG fallback
        else if let Some(path) = Self::try_get_path().ok().filter(|p| p.exists()) {
            builder = builder.add_source(File::from(path));
        }

        // Env overrides (example: `HZRD_SUBMITTER_CONFIG_TOKEN`)
        builder = builder.add_source(Environment::with_prefix("HZRD").separator("_"));

        // CLI overrides
        match &cli.command {
            Commands::Attack(args) => {
                if let Some(hosts) = &args.hosts {
                    builder = builder.set_override("submitter.config.hosts", hosts.to_owned())?;
                }
            }
        }

        let built = builder.build()?;
        Ok(built.try_deserialize()?)
    }

    fn try_get_path() -> Result<PathBuf> {
        let base_dir = dirs::config_dir().ok_or(ConfigError::ConfigDirNotFound)?;
        Ok(base_dir.join("hzrd").join("hzrd.toml"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            attacker: AttackerConfig {
                exploit: Path::new("exploits").into(),
                flag: "[0-9A-Z]{31}=".to_string(),
                teams: HashMap::new(),
                r#loop: Some(AttackerLoopConfig {
                    every: 120,
                    random: Some(10),
                }),
            },
            submitter: Some(SubmitterConfig {
                r#type: "tcp".to_string(),
                database: DatabaseConfig {
                    file: "flags.sqlite".to_string(),
                },
                config: SubmissionConfig {
                    tcp: Some(SubmitterTCPConfig {
                        host: "127.0.0.1".parse().unwrap(),
                        port: 1337,
                        token: "changeme".to_string(),
                    }),
                    http: None,
                },
            }),
        }
    }
}

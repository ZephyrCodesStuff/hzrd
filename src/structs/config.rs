use anyhow::Result;
use config::{Config as RawConfig, Environment, File};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use url::Url;

use std::collections::HashMap;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::cli::{Args, Commands};
use crate::progress_bar;

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

    /// Available exploits (populated at runtime)
    #[serde(skip)]
    pub exploits: Vec<ExploitInfo>,
}

/// Information about an individual exploit script
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExploitInfo {
    /// Path to the exploit script
    pub path: PathBuf,

    /// Filename of the exploit (for display purposes)
    pub name: String,

    /// Whether this exploit is enabled
    pub enabled: bool,
}

impl ExploitInfo {
    /// Create a new ExploitInfo from a path
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            path,
            name,
            enabled: true, // Enabled by default
        }
    }

    /// Toggle the enabled status
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
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
#[serde(rename_all = "lowercase")]
pub enum SubmitterType {
    /// TCP submitter
    Tcp,

    /// HTTP submitter
    Http,
}

impl Display for SubmitterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp => write!(f, "tcp"),
            Self::Http => write!(f, "http"),
        }
    }
}

impl FromStr for SubmitterType {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Self::Tcp),
            "http" => Ok(Self::Http),
            _ => Err(ConfigError::InvalidSubmitterType(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubmitterConfig {
    /// Chosen configuration type
    pub r#type: SubmitterType,

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

    /// Accept insecure TLS connections (e.g. self-signed certs).
    ///
    /// Defaults to `false`.
    #[serde(default)]
    pub insecure: bool,

    /// Timeout for the HTTP request in seconds.
    ///
    /// Defaults to `60`.
    #[serde(default)]
    pub timeout: Timeout,
}

/// Timeout in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Timeout(pub usize);

impl Default for Timeout {
    fn default() -> Self {
        Self(60)
    }
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
            builder = builder.add_source(File::with_name("hzrd.toml"));
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
                    debug!("Detected `--hosts` override: {:?}", hosts);

                    // TODO: actually override teams and not just merge
                    //       (fuck the `config` crate's authors)
                    warn!("WARNING: This does probably not do what you think.");
                    warn!(
                        "The `--hosts` flag will actually MERGE your existing teams with new temporary ones."
                    );
                    warn!(
                        "This means that YOUR EXPLOITS WILL RUN on the existing teams' machines."
                    );
                    warn!("If you don't want this, please remove them from your config.");
                    warn!("Waiting 10 seconds before proceeding.");

                    // Wait 10 seconds for the user to read the warning
                    let pb = progress_bar!(10);
                    for _ in 0..10 {
                        std::thread::sleep(std::time::Duration::from_millis(1000));
                        pb.inc(1);
                    }

                    // Create teams from the host list with names `override_host_<i>`
                    let teams: HashMap<String, Team> = hosts
                        .iter()
                        .enumerate()
                        .filter_map(|(i, host)| match host.to_owned().parse() {
                            Ok(ip) => Some((format!("extra_team_{i}"), Team { ip, nop: None })),
                            Err(err) => {
                                error!("Failed to parse IP address for extra host {host}: {err}");
                                None
                            }
                        })
                        .collect();

                    // Add more teams
                    builder = builder.set_override("attacker.teams", teams)?;
                }
            }
            Commands::Display => {}
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
                exploits: Vec::new(),
            },
            submitter: Some(SubmitterConfig {
                r#type: SubmitterType::Tcp,
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

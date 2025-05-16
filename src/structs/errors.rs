use std::path::PathBuf;

use thiserror::Error;

/// Errors that may happen while fetching the configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to parse TOML config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Could not determine config directory")]
    ConfigDirNotFound,

    #[error("The specified submitter type is invalid: {0}")]
    InvalidSubmitterType(String),
}

/// Errors that may happen while submitting a flag.
#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("Could not initialise SQLite database")]
    SQLiteInit(rusqlite::Error),

    #[error("Could not retrieve flags from database")]
    RetrieveFlags(rusqlite::Error),

    #[error("Failed to connect to the service")]
    ServiceConnection(std::io::Error),

    #[error("Failed to communicate with the service")]
    ServiceCommunication(std::io::Error),

    #[error("No submitter configuration found")]
    NoSubmitter,

    /// No greeting detected -- submitter may be broken
    #[error("No greeting detected")]
    NoGreeting,

    /// No ready message detected -- submitter may have failed authenticating
    #[error("No ready message detected")]
    NoReadyMessage,

    /// Database error: failed to update flags
    #[error("Database error: failed to update flags")]
    Database(rusqlite::Error),
}

/// Errors that may happen while attacking
#[derive(Debug, Error, Clone)]
pub enum AttackError {
    #[error("The specified exploit path does not exist ({0})")]
    NoSuchExploit(PathBuf),

    #[error("The specified team does not exist ({0})")]
    NoSuchTeam(String),

    #[error("We didn't manage to capture any flags")]
    NoCaptures,

    #[error("Script execution error in {script}: {message}")]
    ScriptExecutionError { script: String, message: String },
}

/// Errors that may happen while displaying the dashboard.
#[derive(Debug, Error)]
pub enum DisplayError {
    // TODO: make more specific
    #[error("No submitter configuration found")]
    NoSubmitter,

    #[error("Failed to connect to the service")]
    Display(std::io::Error),

    #[error("Failed to initialise terminal")]
    Rusqlite(rusqlite::Error),
}

use thiserror::Error;

/// Errors that may happen while fetching the configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to parse TOML config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Could not determine config directory")]
    ConfigDirNotFound,
}

/// Errors that may happen while submitting a flag.
#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("Could not initialise SQLite database")]
    SQLiteInitError(rusqlite::Error),

    #[error("Could not store flags in database")]
    StoreFlagsError(rusqlite::Error),

    #[error("Could not retrieve flags from database")]
    RetrieveFlagsError(rusqlite::Error),

    #[error("Failed to connect to the service")]
    ServiceConnectionError(std::io::Error),

    #[error("Failed to communicate with the service")]
    ServiceCommunicationError(std::io::Error),

    /// No greeting detected -- submitter may be broken
    #[error("No greeting detected")]
    NoGreetingError,

    /// No ready message detected -- submitter may have failed authenticating
    #[error("No ready message detected")]
    NoReadyMessageError,

    /// Database error: failed to update flags
    #[error("Database error: failed to update flags")]
    DatabaseError(rusqlite::Error),
}

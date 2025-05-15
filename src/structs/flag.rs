use std::fmt::Display;

use serde::Deserialize;

// FlagStatus represents the possible states of a flag in our system
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FlagStatus {
    Pending,  // Not yet submitted to the scoring system
    Accepted, // Successfully submitted and accepted
    Rejected, // Submitted but rejected (invalid, too old, etc.)
    Error,    // Error occurred during submission
}

impl From<SubmitterHTTPStatus> for FlagStatus {
    fn from(status: SubmitterHTTPStatus) -> Self {
        match status {
            SubmitterHTTPStatus::Accepted => Self::Accepted,
            SubmitterHTTPStatus::Denied => Self::Rejected,
            SubmitterHTTPStatus::Resubmit => Self::Pending,
            SubmitterHTTPStatus::Error => Self::Error,
        }
    }
}

impl Display for FlagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Accepted => write!(f, "Accepted"),
            Self::Rejected => write!(f, "Rejected"),
            Self::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SubmitterHTTPStatus {
    Accepted,
    Denied,
    Resubmit,
    Error,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitterHTTPResponse {
    #[serde(rename = "msg")]
    pub message: String,
    pub flag: String,
    pub status: SubmitterHTTPStatus,
}

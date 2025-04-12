// FlagStatus represents the possible states of a flag in our system
#[derive(Debug, Clone, PartialEq)]
pub enum FlagStatus {
    Pending,  // Not yet submitted to the scoring system
    Accepted, // Successfully submitted and accepted
    Rejected, // Submitted but rejected (invalid, too old, etc.)
    Error,    // Error occurred during submission
}

impl FlagStatus {
    pub fn to_string(&self) -> String {
        match self {
            FlagStatus::Pending => "pending".to_string(),
            FlagStatus::Accepted => "accepted".to_string(),
            FlagStatus::Rejected => "rejected".to_string(),
            FlagStatus::Error => "error".to_string(),
        }
    }
}

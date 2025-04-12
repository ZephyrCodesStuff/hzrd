use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Team {
    /// IP address of the team's "vulnbox"
    pub ip: Ipv4Addr,

    /// Whether the team is a `NOP` (NOn-Playing) team.
    ///
    /// This is used when testing the exploits without running on enemy machines,
    /// so as not to reveal your payloads.
    pub nop: Option<bool>,
}

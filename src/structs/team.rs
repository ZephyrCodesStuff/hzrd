use std::{collections::HashMap, net::Ipv4Addr};

use config::{Value, ValueKind};
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

impl From<Team> for ValueKind {
    fn from(value: Team) -> Self {
        let mut map: HashMap<String, Value> = HashMap::new();

        map.insert(String::from("ip"), Value::from(value.ip.to_string()));
        map.insert(String::from("nop"), Value::from(value.nop));

        Self::Table(map)
    }
}

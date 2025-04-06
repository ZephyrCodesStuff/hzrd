use std::{fmt::Display, str::FromStr};

use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};

/// De/serializable version of `Ipv4Net`
#[derive(Debug, Clone)]
pub struct Subnet(pub Ipv4Net);
impl<'de> Deserialize<'de> for Subnet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
        D::Error: serde::de::Error,
    {
        let subnet_str = String::deserialize(deserializer)?;
        let subnet = Ipv4Net::from_str(&subnet_str)
            .map_err(|e| serde::de::Error::custom(format!("{}", e)))?;
        Ok(Subnet(subnet))
    }
}

impl Serialize for Subnet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl Display for Subnet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Subnet {
    type Err = ipnet::AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let subnet = Ipv4Net::from_str(s)?;
        Ok(Subnet(subnet))
    }
}

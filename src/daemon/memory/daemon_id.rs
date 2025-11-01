use std::{fmt::Display, ops::Deref, str::FromStr};

use anyhow::{Context, Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct DaemonId(u128);

impl DaemonId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().as_u128())
    }
}

impl Deref for DaemonId {
    type Target = u128;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for DaemonId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse().context("Failed to parse daemon ID.")?))
    }
}

impl Display for DaemonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

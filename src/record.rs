use std::{num::ParseFloatError, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub timestamp: Seconds,
    pub value: serde_json::Value,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(from = "f64", into = "f64")]
pub struct Seconds(Duration);

impl Seconds {
    pub const fn new(seconds: Duration) -> Self {
        Self(seconds)
    }

    pub const fn get(self) -> Duration {
        self.0
    }
}

impl From<Seconds> for f64 {
    fn from(value: Seconds) -> Self {
        value.0.as_secs_f64()
    }
}

impl From<f64> for Seconds {
    fn from(value: f64) -> Self {
        Self(Duration::from_secs_f64(value))
    }
}

impl FromStr for Seconds {
    type Err = ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: f64 = s.parse()?;
        Ok(v.into())
    }
}

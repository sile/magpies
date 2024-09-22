use std::{
    collections::BTreeMap,
    num::ParseFloatError,
    str::FromStr,
    time::{Duration, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub target: String,
    pub timestamp: Seconds,
    pub value: serde_json::Value,
}

impl Record {
    pub fn flatten(&self) -> FlattenedRecord {
        let mut items = BTreeMap::new();
        flatten_json_value(&self.value, &mut String::new(), &mut items);
        FlattenedRecord {
            target: self.target.clone(),
            timestamp: self.timestamp.get(),
            items,
        }
    }
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

    pub fn timestamp() -> Self {
        Self::new(UNIX_EPOCH.elapsed().unwrap_or_default())
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

#[derive(Debug, Clone)]
pub struct FlattenedRecord {
    pub target: String,
    pub timestamp: Duration,
    pub items: BTreeMap<String, ItemValue>,
}

#[derive(Debug, Clone)]
pub enum ItemValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

fn flatten_json_value(
    value: &serde_json::Value,
    key: &mut String,
    items: &mut BTreeMap<String, ItemValue>,
) {
    match value {
        serde_json::Value::Null => {
            items.insert(key.clone(), ItemValue::Null);
        }
        serde_json::Value::Bool(v) => {
            items.insert(key.clone(), ItemValue::Bool(*v));
        }
        serde_json::Value::Number(v) => {
            if let Some(v) = v.as_f64() {
                items.insert(key.clone(), ItemValue::Float(v));
            } else if let Some(v) = v.as_i64() {
                items.insert(key.clone(), ItemValue::Integer(v));
            } else if let Some(v) = v.as_u64() {
                items.insert(key.clone(), ItemValue::Integer(v as i64));
            } else {
                unreachable!();
            }
        }
        serde_json::Value::String(v) => {
            items.insert(key.clone(), ItemValue::String(v.clone()));
        }
        serde_json::Value::Array(vs) => {
            let len = key.len();
            let width = vs.len().to_string().len();
            for (i, value) in vs.iter().enumerate() {
                if !key.is_empty() {
                    key.push('.');
                }
                println!("{i:0width$}");
                flatten_json_value(value, key, items);
                key.truncate(len);
            }
        }
        serde_json::Value::Object(vs) => {
            let len = key.len();
            for (name, value) in vs {
                if !key.is_empty() {
                    key.push('.');
                }
                key.push_str(name);
                flatten_json_value(value, key, items);
                key.truncate(len);
            }
        }
    }
}

use std::{
    collections::BTreeMap,
    num::ParseIntError,
    str::FromStr,
    time::{Duration, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub target: String,
    pub timestamp: SecondsF64,
    pub value: serde_json::Value,
}

impl Record {
    pub fn flatten(&self) -> FlattenedRecord {
        let mut items = Items::new();
        flatten_json_value(&self.value, &mut String::new(), &mut items);
        FlattenedRecord {
            target: self.target.clone(),
            timestamp: self.timestamp.to_duration(),
            items,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct SecondsF64(f64);

impl SecondsF64 {
    pub fn to_duration(self) -> Duration {
        Duration::from_secs_f64(self.0)
    }

    pub fn timestamp() -> Self {
        Self(UNIX_EPOCH.elapsed().unwrap_or_default().as_secs_f64())
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct SecondsU64(u64);

impl SecondsU64 {
    pub const fn new(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn to_duration(self) -> Duration {
        Duration::from_secs(self.0)
    }
}

impl FromStr for SecondsU64 {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: u64 = s.parse()?;
        Ok(Self(v))
    }
}

#[derive(Debug, Clone)]
pub struct FlattenedRecord {
    pub target: String,
    pub timestamp: Duration,
    pub items: Items,
}

#[derive(Debug, Clone)]
pub enum ItemValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

fn flatten_json_value(value: &serde_json::Value, key: &mut String, items: &mut Items) {
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

pub type Items = BTreeMap<String, ItemValue>;

#[derive(Debug, Clone)]
pub struct TimeSeries {
    pub start_time: Duration,
    pub segment_duration: Duration,
    pub segments: Vec<TimeSeriesSegment>,
}

impl TimeSeries {
    pub fn new(segment_duration: Duration) -> Self {
        Self {
            start_time: Duration::ZERO,
            segment_duration,
            segments: Vec::new(),
        }
    }

    pub fn insert(&mut self, record: &Record) {
        let record = record.flatten();
        if self.segments.is_empty() || record.timestamp < self.start_time {
            self.start_time = record.timestamp;
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeSeriesSegment {
    pub start_time: Duration,
    pub end_time: Duration,
    pub aggregated_items: Items,
    pub target_items: BTreeMap<String, Items>,
}

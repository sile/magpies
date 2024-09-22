use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU64, ParseIntError},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SecondsNonZeroU64(NonZeroU64);

impl SecondsNonZeroU64 {
    pub const fn new(seconds: NonZeroU64) -> Self {
        Self(seconds)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub const fn to_duration(self) -> Duration {
        Duration::from_secs(self.0.get())
    }
}

impl FromStr for SecondsNonZeroU64 {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: NonZeroU64 = s.parse()?;
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

impl PartialOrd for ItemValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ItemValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Integer(a), Self::Integer(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
            (Self::String(a), Self::String(b)) => a.cmp(b),
            (Self::Null, _) => Ordering::Less,
            (_, Self::Null) => Ordering::Greater,
            (Self::Bool(_), _) => Ordering::Less,
            (_, Self::Bool(_)) => Ordering::Greater,
            (Self::Integer(_), _) => Ordering::Less,
            (_, Self::Integer(_)) => Ordering::Greater,
            (Self::Float(_), _) => Ordering::Less,
            (_, Self::Float(_)) => Ordering::Greater,
        }
    }
}

impl PartialEq for ItemValue {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for ItemValue {}

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

// TODO
pub type Items = BTreeMap<String, ItemValue>;

#[derive(Debug, Clone)]
pub struct TimeSeries {
    pub start_time: SecondsU64,
    pub segment_duration: SecondsNonZeroU64,
    pub segments: BTreeMap<SecondsU64, TimeSeriesSegment>,
    pub dirty_segments: BTreeSet<SecondsU64>,
}

impl TimeSeries {
    pub fn new(segment_duration: SecondsNonZeroU64) -> Self {
        Self {
            start_time: SecondsU64::new(0),
            segment_duration,
            segments: BTreeMap::new(),
            dirty_segments: BTreeSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn insert(&mut self, record: &Record) {
        let record = record.flatten();

        let start_time = record.timestamp.as_secs();
        let start_time = SecondsU64::new(start_time - start_time % self.segment_duration.get());
        let end_time = SecondsU64::new(start_time.get() + 1);
        if self.segments.is_empty() || start_time < self.start_time {
            self.start_time = start_time;
        }

        let segment = self
            .segments
            .entry(start_time)
            .or_insert_with(|| TimeSeriesSegment {
                start_time,
                end_time,
                aggregated_values: BTreeMap::new(),
                target_segment_values: BTreeMap::new(),
            });
        let target_segment = segment
            .target_segment_values
            .entry(record.target)
            .or_default();
        for (key, value) in record.items {
            target_segment
                .entry(key)
                .or_default()
                .raw_values
                .push(value);
        }

        self.dirty_segments.insert(start_time);
    }

    pub fn last_time(&self) -> SecondsU64 {
        self.segments
            .last_key_value()
            .map(|x| *x.0)
            .unwrap_or_default()
    }

    pub fn sync_state(&mut self) {
        for start_time in std::mem::take(&mut self.dirty_segments) {
            //
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeSeriesSegment {
    pub start_time: SecondsU64,
    pub end_time: SecondsU64,
    pub aggregated_values: BTreeMap<String, AggregatedValue>,
    pub target_segment_values: BTreeMap<String, BTreeMap<String, SegmentValue>>,
}

#[derive(Debug, Clone)]
pub struct AggregatedValue {
    pub sum: Option<serde_json::Number>,
    pub delta: Option<serde_json::Number>,
}

#[derive(Debug, Default, Clone)]
pub struct SegmentValue {
    pub value: Option<RepresentativeValue>,
    pub delta: Option<serde_json::Number>,
    pub raw_values: Vec<ItemValue>,
}

#[derive(Debug, Clone)]
pub enum RepresentativeValue {
    Avg(serde_json::Number),
    Set(BTreeSet<ItemValue>),
}

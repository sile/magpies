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

impl ItemValue {
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Float(_))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Integer(v) = self {
            Some(*v as f64)
        } else if let Self::Float(v) = self {
            Some(*v)
        } else {
            None
        }
    }
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
            if let Some(v) = v.as_i64() {
                items.insert(key.clone(), ItemValue::Integer(v));
            } else if let Some(_) = v.as_u64() {
                items.insert(key.clone(), ItemValue::Integer(i64::MAX));
            } else if let Some(v) = v.as_f64() {
                items.insert(key.clone(), ItemValue::Float(v));
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
        if self.segments.is_empty() || start_time < self.start_time {
            self.start_time = start_time;
        }

        let segment = self
            .segments
            .entry(start_time)
            .or_insert_with(|| TimeSeriesSegment {
                start_time,
                end_time: SecondsU64::new(start_time.get() + self.segment_duration.get()),
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
        let empty_segment = TimeSeriesSegment::empty(self.segment_duration);
        for start_time in std::mem::take(&mut self.dirty_segments) {
            let prev_segment = self.segments.get(&start_time).unwrap_or(&empty_segment);
            let mut segment = self.segments.get(&start_time).expect("unreachable").clone();
            segment.sync_state(prev_segment);
            self.segments.insert(start_time, segment);
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

impl TimeSeriesSegment {
    pub fn empty(segment_duration: SecondsNonZeroU64) -> Self {
        Self {
            start_time: SecondsU64::new(0),
            end_time: SecondsU64::new(segment_duration.get()),
            aggregated_values: BTreeMap::new(),
            target_segment_values: BTreeMap::new(),
        }
    }

    fn sync_state(&mut self, prev_segment: &Self) {
        self.sync_target_segment_values(prev_segment);
        self.sync_aggregated_values(prev_segment);
    }

    fn sync_target_segment_values(&mut self, prev_segment: &Self) {
        for (target, segment_values) in &mut self.target_segment_values {
            for (key, segment_value) in segment_values {
                segment_value.sync_representative_value();
                if let Some(prev_segment_value) = prev_segment
                    .target_segment_values
                    .get(target)
                    .and_then(|v| v.get(key))
                {
                    segment_value.sync_delta(prev_segment_value);
                }
            }
        }
    }

    fn sync_aggregated_values(&mut self, prev_segment: &Self) {
        for (key, aggregated_value) in &mut self.aggregated_values {
            let mut sum = None;
            for value in self
                .target_segment_values
                .values()
                .filter_map(|segment_values| segment_values.get(key).map(|v| &v.value))
            {
                match (value, sum) {
                    (value, None) => {
                        sum = Some(value.clone());
                    }
                    (RepresentativeValue::Avg(_), Some(RepresentativeValue::Set(_))) => {
                        sum = None;
                        continue;
                    }
                    (RepresentativeValue::Set(_), Some(RepresentativeValue::Avg(_))) => {
                        sum = None;
                        continue;
                    }
                    (RepresentativeValue::Avg(a), Some(RepresentativeValue::Avg(b))) => {
                        if let Some(v) = number_add(a.clone(), b.clone()) {
                            sum = Some(RepresentativeValue::Avg(v));
                        } else {
                            sum = None;
                            continue;
                        }
                    }
                    (RepresentativeValue::Set(a), Some(RepresentativeValue::Set(mut b))) => {
                        b.extend(a.iter().cloned());
                        sum = Some(RepresentativeValue::Set(b));
                    }
                }
            }
            aggregated_value.sum = sum;

            let Some(RepresentativeValue::Avg(v0)) = prev_segment
                .aggregated_values
                .get(key)
                .and_then(|v| v.sum.as_ref())
            else {
                continue;
            };
            let Some(RepresentativeValue::Avg(v1)) = &aggregated_value.sum else {
                continue;
            };
            aggregated_value.delta = number_sub(v1.clone(), v0.clone());
        }
    }
}

fn number_sub(a: serde_json::Number, b: serde_json::Number) -> Option<serde_json::Number> {
    apply_number_op(a, b, |a, b| a - b, |a, b| a - b)
}

fn number_add(a: serde_json::Number, b: serde_json::Number) -> Option<serde_json::Number> {
    apply_number_op(a, b, |a, b| a + b, |a, b| a + b)
}

fn apply_number_op<F0, F1>(
    a: serde_json::Number,
    b: serde_json::Number,
    f0: F0,
    f1: F1,
) -> Option<serde_json::Number>
where
    F0: FnOnce(i64, i64) -> i64,
    F1: FnOnce(f64, f64) -> f64,
{
    if let (Some(v0), Some(v1)) = (a.as_i64(), b.as_i64()) {
        Some(serde_json::Number::from(f0(v0, v1)))
    } else if let (Some(v0), Some(v1)) = (a.as_f64(), b.as_f64()) {
        serde_json::Number::from_f64(f1(v0, v1))
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct AggregatedValue {
    pub sum: Option<RepresentativeValue>,
    pub delta: Option<serde_json::Number>,
}

#[derive(Debug, Default, Clone)]
pub struct SegmentValue {
    pub value: RepresentativeValue,
    pub delta: Option<serde_json::Number>,
    pub raw_values: Vec<ItemValue>,
}

impl SegmentValue {
    fn sync_representative_value(&mut self) {
        if self.raw_values.iter().all(|v| v.is_integer()) {
            let sum: i64 = self.raw_values.iter().filter_map(|v| v.as_i64()).sum();
            let avg = sum / self.raw_values.len() as i64;
            self.value = RepresentativeValue::Avg(serde_json::Number::from(avg));
            return;
        } else if self.raw_values.iter().all(|v| v.is_number()) {
            let sum: f64 = self.raw_values.iter().filter_map(|v| v.as_f64()).sum();
            let avg = sum / self.raw_values.len() as f64;
            if let Some(v) = serde_json::Number::from_f64(avg) {
                self.value = RepresentativeValue::Avg(v);
                return;
            }
        }

        self.value = RepresentativeValue::Set(self.raw_values.iter().cloned().collect());
    }

    fn sync_delta(&mut self, prev: &Self) {
        let RepresentativeValue::Avg(v0) = &self.value else {
            return;
        };
        let RepresentativeValue::Avg(v1) = &prev.value else {
            return;
        };

        self.delta = number_sub(v0.clone(), v1.clone());
    }
}

#[derive(Debug, Clone)]
pub enum RepresentativeValue {
    Avg(serde_json::Number),
    Set(BTreeSet<ItemValue>),
}

impl Default for RepresentativeValue {
    fn default() -> Self {
        Self::Set(BTreeSet::new())
    }
}

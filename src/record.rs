use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::num::{fmt_f64, fmt_i64, SecondsF64, SecondsNonZeroU64, SecondsU64};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub target: String,
    pub timestamp: SecondsF64,
    pub value: serde_json::Value,
}

impl Record {
    pub fn flatten(&self) -> FlattenedRecord {
        let mut metrics = BTreeMap::new();
        flatten_json_value(&self.value, &mut String::new(), &mut metrics);
        FlattenedRecord {
            target: self.target.clone(),
            timestamp: self.timestamp.to_duration(),
            metrics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlattenedRecord {
    pub target: String,
    pub timestamp: Duration,
    pub metrics: BTreeMap<String, MetricValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum MetricValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl MetricValue {
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

impl PartialOrd for MetricValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MetricValue {
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

impl PartialEq for MetricValue {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for MetricValue {}

fn flatten_json_value(
    value: &serde_json::Value,
    key: &mut String,
    metrics: &mut BTreeMap<String, MetricValue>,
) {
    match value {
        serde_json::Value::Null => {
            metrics.insert(key.clone(), MetricValue::Null);
        }
        serde_json::Value::Bool(v) => {
            metrics.insert(key.clone(), MetricValue::Bool(*v));
        }
        serde_json::Value::Number(v) => {
            if let Some(v) = v.as_i64() {
                metrics.insert(key.clone(), MetricValue::Integer(v));
            } else if v.as_u64().is_some() {
                metrics.insert(key.clone(), MetricValue::Integer(i64::MAX));
            } else if let Some(v) = v.as_f64() {
                metrics.insert(key.clone(), MetricValue::Float(v));
            } else {
                unreachable!();
            }
        }
        serde_json::Value::String(v) => {
            metrics.insert(key.clone(), MetricValue::String(v.clone()));
        }
        serde_json::Value::Array(vs) => {
            let len = key.len();
            let width = vs.len().to_string().len();
            for (i, value) in vs.iter().enumerate() {
                if !key.is_empty() {
                    key.push('.');
                }
                key.push_str(&format!("{i:0width$}"));
                flatten_json_value(value, key, metrics);
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
                flatten_json_value(value, key, metrics);
                key.truncate(len);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeSeries {
    pub start_time: SecondsU64,
    pub end_time: SecondsU64,
    pub segment_duration: SecondsNonZeroU64,
    pub segments: BTreeMap<SecondsU64, TimeSeriesSegment>,
    pub dirty_segments: BTreeSet<SecondsU64>,
}

impl TimeSeries {
    pub fn new(segment_duration: SecondsNonZeroU64) -> Self {
        Self {
            start_time: SecondsU64::new(0),
            end_time: SecondsU64::new(0),
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
        self.end_time = self.end_time.max(SecondsU64::new(start_time + 1));

        let start_time = SecondsU64::new(start_time - start_time % self.segment_duration.get());
        if self.segments.is_empty() || start_time < self.start_time {
            self.start_time = start_time;
        }

        let segment = self
            .segments
            .entry(start_time)
            .or_insert_with(|| TimeSeriesSegment {
                start_time,
                segment_duration: self.segment_duration,
                aggregated_values: BTreeMap::new(),
                target_segment_values: BTreeMap::new(),
            });
        let target_segment = segment
            .target_segment_values
            .entry(record.target)
            .or_default();
        for (key, value) in record.metrics {
            target_segment
                .entry(key)
                .or_default()
                .raw_values
                .push(value);
        }

        self.dirty_segments.insert(start_time);
    }

    pub fn last_start_time(&self) -> SecondsU64 {
        self.segments
            .last_key_value()
            .map(|x| *x.0)
            .unwrap_or_default()
    }

    pub fn sync_state(&mut self) {
        let empty_segment = TimeSeriesSegment::empty(self.segment_duration);
        for start_time in std::mem::take(&mut self.dirty_segments) {
            let prev_time = start_time
                .get()
                .checked_sub(self.segment_duration.get())
                .map(SecondsU64::new);
            let prev_segment = prev_time
                .and_then(|t| self.segments.get(&t))
                .unwrap_or(&empty_segment);

            let mut segment = self.segments.get(&start_time).expect("unreachable").clone();
            segment.sync_state(prev_segment);
            self.segments.insert(start_time, segment);
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeSeriesSegment {
    pub start_time: SecondsU64,
    pub segment_duration: SecondsNonZeroU64,
    pub aggregated_values: BTreeMap<String, AggregatedValue>,
    pub target_segment_values: BTreeMap<String, BTreeMap<String, SegmentValue>>,
}

impl TimeSeriesSegment {
    pub fn empty(segment_duration: SecondsNonZeroU64) -> Self {
        Self {
            start_time: SecondsU64::new(0),
            segment_duration,
            aggregated_values: BTreeMap::new(),
            target_segment_values: BTreeMap::new(),
        }
    }

    pub fn end_time(&self) -> SecondsU64 {
        SecondsU64::new(self.start_time.get() + self.segment_duration.get())
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
                    segment_value.sync_delta(prev_segment_value, self.segment_duration);
                }
            }
        }
    }

    fn sync_aggregated_values(&mut self, prev_segment: &Self) {
        let keys = self
            .target_segment_values
            .values()
            .flat_map(|segment_values| segment_values.keys())
            .collect::<BTreeSet<_>>();
        for key in keys {
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
                        break;
                    }
                    (RepresentativeValue::Set(_), Some(RepresentativeValue::Avg(_))) => {
                        sum = None;
                        break;
                    }
                    (RepresentativeValue::Avg(a), Some(RepresentativeValue::Avg(b))) => {
                        if let Some(v) = number_add(a.clone(), b.clone()) {
                            sum = Some(RepresentativeValue::Avg(v));
                        } else {
                            sum = None;
                            break;
                        }
                    }
                    (RepresentativeValue::Set(a), Some(RepresentativeValue::Set(mut b))) => {
                        b.extend(a.iter().cloned());
                        sum = Some(RepresentativeValue::Set(b));
                    }
                }
            }

            let mut delta = None;
            if let Some(RepresentativeValue::Avg(v0)) = prev_segment
                .aggregated_values
                .get(key)
                .and_then(|v| v.sum.as_ref())
            {
                if let Some(RepresentativeValue::Avg(v1)) = &sum {
                    delta = number_delta(v1.clone(), v0.clone(), self.segment_duration);
                }
            }
            self.aggregated_values
                .insert(key.clone(), AggregatedValue { sum, delta });
        }
    }
}

fn number_delta(
    a: serde_json::Number,
    b: serde_json::Number,
    d: SecondsNonZeroU64,
) -> Option<serde_json::Number> {
    let d = d.get();
    apply_number_op(a, b, |a, b| (a - b) / d as i64, |a, b| (a - b) / d as f64)
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

impl AggregatedValue {
    pub fn sum_text(&self, decimal_places: u8) -> String {
        let Some(v) = &self.sum else {
            return "".to_owned();
        };
        match v {
            RepresentativeValue::Avg(v) => {
                if let Some(v) = v.as_i64() {
                    fmt_i64(v)
                } else if let Some(v) = v.as_f64() {
                    fmt_f64(v, decimal_places as usize)
                } else {
                    unreachable!()
                }
            }
            RepresentativeValue::Set(vs) => serde_json::to_string(vs).expect("unreachable"),
        }
    }

    pub fn delta_text(&self, decimal_places: u8) -> String {
        let Some(v) = &self.delta else {
            return "".to_owned();
        };
        if let Some(v) = v.as_i64() {
            fmt_i64(v)
        } else if let Some(v) = v.as_f64() {
            fmt_f64(v, decimal_places as usize)
        } else {
            unreachable!()
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SegmentValue {
    pub value: RepresentativeValue,
    pub delta: Option<serde_json::Number>,
    pub raw_values: Vec<MetricValue>,
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

    fn sync_delta(&mut self, prev: &Self, segment_duration: SecondsNonZeroU64) {
        let RepresentativeValue::Avg(v0) = &self.value else {
            return;
        };
        let RepresentativeValue::Avg(v1) = &prev.value else {
            return;
        };

        self.delta = number_delta(v0.clone(), v1.clone(), segment_duration);
    }

    pub fn value_text(&self, decimal_places: u8) -> String {
        match &self.value {
            RepresentativeValue::Avg(v) => {
                if let Some(v) = v.as_i64() {
                    fmt_i64(v)
                } else if let Some(v) = v.as_f64() {
                    fmt_f64(v, decimal_places as usize)
                } else {
                    unreachable!()
                }
            }
            RepresentativeValue::Set(vs) => serde_json::to_string(vs).expect("unreachable"),
        }
    }

    pub fn delta_text(&self, decimal_places: u8) -> String {
        let Some(v) = &self.delta else {
            return "".to_owned();
        };
        if let Some(v) = v.as_i64() {
            fmt_i64(v)
        } else if let Some(v) = v.as_f64() {
            fmt_f64(v, decimal_places as usize)
        } else {
            unreachable!()
        }
    }
}

#[derive(Debug, Clone)]
pub enum RepresentativeValue {
    Avg(serde_json::Number),
    Set(BTreeSet<MetricValue>),
}

impl Default for RepresentativeValue {
    fn default() -> Self {
        Self::Set(BTreeSet::new())
    }
}

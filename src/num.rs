use std::{
    num::{NonZeroU64, ParseIntError},
    str::FromStr,
    time::{Duration, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

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

pub fn fmt_u64(mut n: u64) -> String {
    if n == 0 {
        return n.to_string();
    }

    let mut s = Vec::new();
    let mut i = 0;
    while n > 0 {
        if i > 0 && i % 3 == 0 {
            s.push(',');
        }
        let d = (n % 10) as u8;
        s.push(char::from(b'0' + d));
        n /= 10;
        i += 1;
    }
    s.reverse();
    s.into_iter().collect()
}

pub fn fmt_i64(n: i64) -> String {
    if n < 0 {
        format!("-{}", fmt_u64(n.unsigned_abs()))
    } else {
        fmt_u64(n.unsigned_abs())
    }
}

pub fn fmt_f64(n: f64, decimal_places: usize) -> String {
    let s = format!("{:.1$}", n, decimal_places);
    let mut iter = s.splitn(2, '.');
    let integer = iter.next().expect("unreachable");
    let fraction = iter.next();

    let mut s = Vec::new();
    for (i, c) in integer.chars().rev().enumerate() {
        if c != '-' && i > 0 && i % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }
    s.reverse();

    if let Some(fraction) = fraction {
        s.push('.');
        for (i, c) in fraction.chars().enumerate() {
            if i > 0 && i % 3 == 0 {
                s.push(',');
            }
            s.push(c);
        }
    }

    s.into_iter().collect()
}

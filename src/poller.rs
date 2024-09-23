use std::{
    path::PathBuf,
    process::Command,
    str::FromStr,
    sync::mpsc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{metrics::Record, num::SecondsF64};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollTarget {
    pub target: String,
    pub command_path: PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_args: Vec<String>,
}

impl FromStr for PollTarget {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug)]
pub struct Poller {
    target: PollTarget,
    poll_interval: Duration,
    record_tx: mpsc::Sender<Record>,
    next_poll_time: Instant,
    end_time: Instant,
}

impl Poller {
    pub fn start(
        target: PollTarget,
        poll_interval: Duration,
        poll_duration: Duration,
        record_tx: mpsc::Sender<Record>,
    ) {
        let now = Instant::now();
        let mut poller = Poller {
            target,
            poll_interval,
            record_tx,
            next_poll_time: now,
            end_time: now + poll_duration,
        };
        std::thread::spawn(move || while poller.run_one() {});
    }

    fn run_one(&mut self) -> bool {
        if self.end_time <= self.next_poll_time {
            return false;
        }

        if let Some(value) = self.poll() {
            let record = Record {
                target: self.target.target.clone(),
                timestamp: SecondsF64::timestamp(),
                value,
            };
            if self.record_tx.send(record).is_err() {
                return false;
            }
        }

        let now = Instant::now();
        while self.next_poll_time < now {
            self.next_poll_time += self.poll_interval;
        }
        std::thread::sleep(self.next_poll_time.saturating_duration_since(now));
        true
    }

    fn poll(&self) -> Option<serde_json::Value> {
        match Command::new(&self.target.command_path)
            .args(self.target.command_args.iter())
            .output()
        {
            Err(e) => {
                eprintln!(
                    "[{}] Failed to execute command {:?}: {e}",
                    self.target.target,
                    self.target.command_path.display()
                );
                None
            }
            Ok(output) if !output.status.success() => {
                eprintln!(
                    "[{}] Command {:?} exited abnormaly{}.\n\nSTDOUT:\n{}\n\nSTDERR:{}",
                    self.target.target,
                    self.target.command_path.display(),
                    if let Some(code) = output.status.code() {
                        format!(" with code {code}")
                    } else {
                        "".to_owned()
                    },
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                None
            }
            Ok(output) => match serde_json::from_slice(&output.stdout) {
                Err(e) => {
                    eprintln!(
                        "[{}] Command {:?} output is not JSON: {e}\n\nSTDOUT:{}",
                        self.target.target,
                        self.target.command_path.display(),
                        String::from_utf8_lossy(&output.stdout)
                    );
                    None
                }
                Ok(value) => Some(value),
            },
        }
    }
}

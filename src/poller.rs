use std::{path::PathBuf, str::FromStr, sync::mpsc, time::Duration};

use serde::{Deserialize, Serialize};

use crate::record::Record;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollTarget {
    pub target_name: String,
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
    poll_duration: Duration,
    record_tx: mpsc::Sender<Record>,
}

impl Poller {
    pub fn start(
        target: PollTarget,
        poll_interval: Duration,
        poll_duration: Duration,
        record_tx: mpsc::Sender<Record>,
    ) {
        let poller = Poller {
            target,
            poll_interval,
            poll_duration,
            record_tx,
        };
        std::thread::spawn(move || {
            poller.run();
        });
    }

    fn run(self) {}
}

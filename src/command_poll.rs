use std::sync::mpsc;

use orfail::OrFail;

use crate::{
    num::SecondsU64,
    poller::{PollTarget, Poller},
};

const YEAR: SecondsU64 = SecondsU64::new(364 * 24 * 60 * 60);

#[derive(Debug, clap::Args)]
pub struct PollCommand {
    pub target: PollTarget,
    pub additional_targets: Vec<PollTarget>,

    #[clap(short = 'i', long, default_value = "1")]
    pub poll_interval: SecondsU64,

    #[clap(short, long)]
    pub poll_duration: Option<SecondsU64>,
}

impl PollCommand {
    pub fn run(self) -> orfail::Result<()> {
        let (record_tx, record_rx) = mpsc::channel();

        let poll_duration = self.poll_duration.unwrap_or(YEAR);
        for target in std::iter::once(self.target).chain(self.additional_targets.into_iter()) {
            Poller::start(
                target,
                self.poll_interval.to_duration(),
                poll_duration.to_duration(),
                record_tx.clone(),
            );
        }

        while let Ok(record) = record_rx.recv() {
            println!("{}", serde_json::to_string(&record).or_fail()?);
        }

        Ok(())
    }
}

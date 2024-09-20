use std::{sync::mpsc, time::Duration};

use orfail::OrFail;

use crate::{
    poller::{PollTarget, Poller},
    record::Seconds,
};

const YEAR: Duration = Duration::from_secs(364 * 24 * 60 * 60);

#[derive(Debug, clap::Args)]
pub struct PollCommand {
    pub target: PollTarget,
    pub additional_targets: Vec<PollTarget>,

    #[clap(short = 'i', long, default_value = "1")]
    pub poll_interval: Seconds,

    #[clap(short, long)]
    pub poll_duration: Option<Seconds>,
}

impl PollCommand {
    pub fn run(self) -> orfail::Result<()> {
        let (record_tx, record_rx) = mpsc::channel();

        let poll_duration = self.poll_duration.unwrap_or(Seconds::new(YEAR));
        for target in std::iter::once(self.target).chain(self.additional_targets.into_iter()) {
            Poller::start(
                target,
                self.poll_interval.get(),
                poll_duration.get(),
                record_tx.clone(),
            );
        }

        while let Ok(record) = record_rx.recv() {
            println!("{}", serde_json::to_string(&record).or_fail()?);
        }

        // while self
        //     .poll_duration
        //     .map_or(true, |d| start_time.elapsed() <= d.get())
        // {
        //     // let value = self.poll().or_fail()?;
        //     // let record = Record {
        //     //     target: target.clone(),
        //     //     timestamp: Seconds::new(UNIX_EPOCH.elapsed().or_fail()?),
        //     //     value,
        //     // };

        //     next_poll_time += self.poll_interval.get();
        //     std::thread::sleep(next_poll_time.saturating_duration_since(Instant::now()));
        // }
        Ok(())
    }
}

use std::{
    path::PathBuf,
    time::{Instant, UNIX_EPOCH},
};

use orfail::OrFail;

use crate::record::{Record, Seconds};

#[derive(Debug, clap::Args)]
pub struct PollCommand {
    pub command_name: PathBuf,
    pub command_args: Vec<String>,

    #[clap(short, long)]
    pub target: Option<String>,

    #[clap(short = 'i', long, default_value = "1")]
    pub poll_interval: Seconds,

    #[clap(short, long)]
    pub poll_duration: Option<Seconds>,
}

impl PollCommand {
    pub fn run(mut self) -> orfail::Result<()> {
        let target = self
            .target
            .take()
            .unwrap_or_else(|| format!("pid.{}", std::process::id()));

        let start_time = Instant::now();
        let mut next_poll_time = start_time;
        while self
            .poll_duration
            .map_or(true, |d| start_time.elapsed() <= d.get())
        {
            let value = self.poll().or_fail()?;
            let record = Record {
                target: target.clone(),
                timestamp: Seconds::new(UNIX_EPOCH.elapsed().or_fail()?),
                value,
            };
            println!("{}", serde_json::to_string(&record).or_fail()?);

            next_poll_time += self.poll_interval.get();
            std::thread::sleep(next_poll_time.saturating_duration_since(Instant::now()));
        }
        Ok(())
    }

    fn poll(&self) -> orfail::Result<serde_json::Value> {
        todo!();
    }
}

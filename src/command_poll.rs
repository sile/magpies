use std::path::PathBuf;

use crate::record::Seconds;

#[derive(Debug, clap::Args)]
pub struct PollCommand {
    pub command_name: PathBuf,
    pub command_args: Vec<String>,

    #[clap(short = 'i', long, default_value = "1")]
    pub poll_interval: Seconds,
}

impl PollCommand {
    pub fn run(self) -> orfail::Result<()> {
        todo!()
    }
}

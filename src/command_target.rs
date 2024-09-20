use std::path::PathBuf;

use orfail::OrFail;

use crate::poller::PollTarget;

#[derive(Debug, clap::Args)]
pub struct TargetCommand {
    pub command_path: PathBuf,
    pub command_args: Vec<String>,

    #[clap(short, long)]
    pub target_name: Option<String>,
}

impl TargetCommand {
    pub fn run(mut self) -> orfail::Result<()> {
        let target_name = self
            .target_name
            .take()
            .unwrap_or_else(|| format!("target.{}", std::process::id()));

        let target = PollTarget {
            target_name,
            command_path: self.command_path,
            command_args: self.command_args,
        };
        println!("{}", serde_json::to_string(&target).or_fail()?);
        Ok(())
    }
}

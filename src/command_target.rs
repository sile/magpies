use std::path::PathBuf;

use orfail::OrFail;

use crate::poller::PollTarget;

/// Generate a JSON object that defines a polling target.
#[derive(Debug, clap::Args)]
pub struct TargetCommand {
    /// Path for the command to poll the metrics of the target.
    pub command_path: PathBuf,

    /// Arguments for the command.
    pub command_args: Vec<String>,

    /// The target name. If omitted, `target.${RANDOM_NUMBER}` will be used instead.
    #[clap(short, long)]
    pub target: Option<String>,
}

impl TargetCommand {
    pub fn run(mut self) -> orfail::Result<()> {
        let target = self
            .target
            .take()
            .unwrap_or_else(|| format!("target.{}", std::process::id()));

        let target = PollTarget {
            target,
            command_path: self.command_path,
            command_args: self.command_args,
        };
        println!("{}", serde_json::to_string(&target).or_fail()?);
        Ok(())
    }
}

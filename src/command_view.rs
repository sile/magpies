use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct ViewCommand {
    record_jsonl_file: PathBuf,

    #[clap(short, long)]
    realtime: bool,
}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        Ok(())
    }
}

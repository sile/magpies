use std::path::PathBuf;

use orfail::OrFail;

use crate::{
    jsonl::JsonlReader,
    viewer::{Viewer, ViewerOptions},
};

#[derive(Debug, clap::Args)]
pub struct ViewCommand {
    record_jsonl_file: PathBuf,

    #[clap(short, long)]
    realtime: bool,
}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        let file = std::fs::File::open(&self.record_jsonl_file).or_fail()?;
        let reader = JsonlReader::new(file);
        let options = ViewerOptions {
            realtime: self.realtime,
        };
        let app = Viewer::new(reader, options).or_fail()?;
        app.run().or_fail()?;
        Ok(())
    }
}

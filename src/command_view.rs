use std::{path::PathBuf, time::Duration};

use orfail::OrFail;

use crate::{
    jsonl::JsonlReader,
    record::Record,
    viewer::{ViewerApp, ViewerOptions},
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
        let mut reader = JsonlReader::new(file);
        loop {
            let Some(record) = reader.read_item::<Record>().or_fail()? else {
                std::thread::sleep(Duration::from_millis(100));
                //continue;
                break;
            };
        }

        let options = ViewerOptions {
            realtime: self.realtime,
        };
        let app = ViewerApp::new(options).or_fail()?;
        app.run().or_fail()?;
        Ok(())
    }
}

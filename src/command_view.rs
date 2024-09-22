use std::path::PathBuf;

use orfail::OrFail;
use regex::Regex;

use crate::{
    jsonl::JsonlReader,
    record::Seconds,
    viewer::{Viewer, ViewerOptions},
};

#[derive(Debug, clap::Args)]
pub struct ViewCommand {
    record_jsonl_file: PathBuf,

    #[clap(short, long)]
    realtime: bool,

    #[clap(short, long, default_value = "1")]
    interval: Seconds,

    #[clap(short = 'w', long, default_value = "60")]
    chart_time_window: Seconds,

    #[clap(short = 'f', long, default_value = ".*")]
    item_filter: Regex,
}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        let file = std::fs::File::open(&self.record_jsonl_file).or_fail()?;
        let reader = JsonlReader::new(file);
        let options = ViewerOptions {
            realtime: self.realtime,
            interval: self.interval.get(),
            chart_time_window: self.chart_time_window.get(),
            item_filter: self.item_filter,
        };
        let app = Viewer::new(reader, options).or_fail()?;
        app.run().or_fail()?;
        Ok(())
    }
}

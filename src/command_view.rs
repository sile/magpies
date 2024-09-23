use std::path::PathBuf;

use orfail::OrFail;
use ratatui::symbols::Marker;
use regex::Regex;

use crate::{
    jsonl::JsonlReader,
    record::SecondsNonZeroU64,
    viewer::{Viewer, ViewerOptions},
};

#[derive(Debug, clap::Args)]
pub struct ViewCommand {
    record_jsonl_file: PathBuf,

    #[clap(short, long)]
    absolute_time: bool,

    #[clap(short, long, default_value = "1")]
    interval: SecondsNonZeroU64,

    #[clap(short = 'w', long, default_value = "60")]
    chart_time_window: SecondsNonZeroU64,

    #[clap(short, long, default_value_t = 3)]
    decimal_places: u8,

    #[clap(short = 'f', long, default_value = ".*")]
    item_filter: Regex,

    #[clap(short, long)]
    portable_chart: bool,
}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        let file = std::fs::File::open(&self.record_jsonl_file).or_fail()?;
        let reader = JsonlReader::new(file);
        let options = ViewerOptions {
            absolute_time: self.absolute_time,
            interval: self.interval,
            chart_time_window: self.chart_time_window,
            decimal_places: self.decimal_places,
            item_filter: self.item_filter,
            chart_marker: if self.portable_chart {
                Marker::Dot
            } else {
                Marker::Braille
            },
        };
        let app = Viewer::new(reader, options).or_fail()?;
        app.run().or_fail()?;
        Ok(())
    }
}

use std::path::PathBuf;

use orfail::OrFail;
use ratatui::symbols::Marker;
use regex::Regex;

use crate::{
    jsonl::JsonlReader,
    num::SecondsNonZeroU64,
    viewer::{Viewer, ViewerOptions},
};

/// Launch the TUI viewer to visualize the results of the `poll` command.
#[derive(Debug, clap::Args)]
pub struct ViewCommand {
    /// Path to the file that contains the outputs from executing the `poll` command.
    metrics_jsonl_file: PathBuf,

    /// Time interval in seconds. Metrics within the same interval are grouped together.
    #[clap(short, long, default_value = "1")]
    interval: SecondsNonZeroU64,

    /// Time window in the chart in seconds.
    #[clap(short = 'w', long, default_value = "60")]
    chart_time_window: SecondsNonZeroU64,

    /// Regex pattern specifying metrics that include the visualization.
    #[clap(short = 'f', long, default_value = ".*")]
    metric_filter: Regex,

    /// Number of decimal places when formatting floating-point values.
    #[clap(short, long, default_value_t = 3)]
    decimal_places: u8,

    /// If specified, the chart will be plotted using coarse-grained but highly portable characters.
    #[clap(short, long)]
    portable_chart: bool,

    /// If specified, the viewer shows the absolute time instead of the relative time from the first metric.
    #[clap(short, long)]
    absolute_time: bool,
}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        let file = std::fs::File::open(&self.metrics_jsonl_file).or_fail()?;
        let reader = JsonlReader::new(file);
        let options = ViewerOptions {
            absolute_time: self.absolute_time,
            interval: self.interval,
            chart_time_window: self.chart_time_window,
            decimal_places: self.decimal_places,
            metric_filter: self.metric_filter,
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

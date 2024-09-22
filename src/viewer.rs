use std::{fs::File, time::Duration};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use orfail::OrFail;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    prelude::{Buffer, Rect},
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{block::Title, Block, Paragraph, Widget},
    DefaultTerminal,
};
use regex::Regex;

use crate::{
    jsonl::JsonlReader,
    record::{Record, SecondsNonZeroU64, SecondsU64, TimeSeries, TimeSeriesSegment},
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct ViewerOptions {
    pub absolute_time: bool,
    pub interval: SecondsNonZeroU64,
    pub chart_time_window: SecondsNonZeroU64,
    pub item_filter: Regex,
}

#[derive(Debug)]
pub struct Viewer {
    terminal: DefaultTerminal,
    reader: JsonlReader<File>,
    exit: bool,
    app: ViewerApp,
}

impl Viewer {
    pub fn new(mut reader: JsonlReader<File>, options: ViewerOptions) -> orfail::Result<Self> {
        let mut terminal = ratatui::init();
        terminal.clear().or_fail()?;

        let mut app = ViewerApp::new(&options);
        while let Some(record) = reader.read_item::<Record>().or_fail()? {
            app.insert_record(&record);
        }

        Ok(Self {
            terminal,
            reader,
            exit: false,
            app,
        })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        self.draw().or_fail()?;

        while !self.exit {
            let mut need_redraw = false;
            if event::poll(POLL_INTERVAL).or_fail()? {
                if let event::Event::Key(key) = event::read().or_fail()? {
                    if self.handle_key_event(key).or_fail()? {
                        need_redraw = true;
                    }
                }
            }

            while let Some(record) = self.reader.read_item().or_fail()? {
                self.app.insert_record(&record);
                need_redraw = true;
            }

            if need_redraw {
                self.draw().or_fail()?;
            }
        }

        Ok(())
    }

    fn draw(&mut self) -> orfail::Result<()> {
        self.app.sync_state();
        self.terminal
            .draw(|frame| frame.render_widget(&self.app, frame.area()))
            .or_fail()?;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> orfail::Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }
        if key.code == KeyCode::Char('q') {
            self.exit = true;
        }
        Ok(false)
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[derive(Debug)]
pub struct ViewerApp {
    options: ViewerOptions,
    ts: TimeSeries,
    current_time: SecondsU64,
    base_time: SecondsU64,
    initialized: bool,
    empty_segment: TimeSeriesSegment,
}

impl ViewerApp {
    fn new(options: &ViewerOptions) -> Self {
        Self {
            options: options.clone(),
            ts: TimeSeries::new(options.interval),
            current_time: SecondsU64::new(0),
            base_time: SecondsU64::new(0),
            initialized: false,
            empty_segment: TimeSeriesSegment::empty(options.interval),
        }
    }

    fn insert_record(&mut self, record: &Record) {
        self.ts.insert(record);
    }

    fn sync_state(&mut self) {
        if self.ts.is_empty() {
            return;
        }

        if !self.initialized {
            self.current_time = self.ts.start_time;
            self.initialized = true;
        }

        if !self.options.absolute_time {
            self.base_time = self.ts.start_time;
        }

        self.ts.sync_state();

        let prev_last_start_time = self
            .ts
            .last_start_time()
            .get()
            .checked_sub(self.options.interval.get());
        if Some(self.current_time.get()) == prev_last_start_time {
            self.current_time = self.ts.last_start_time();
        }
    }

    fn calculate_layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
        let [header_area, main_area] =
            Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).areas(area);
        let [status_area, help_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(header_area);
        let [aggregation_area, main_right_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(main_area);
        let [values_area, chart_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(main_right_area);
        (
            status_area,
            help_area,
            aggregation_area,
            values_area,
            chart_area,
        )
    }

    fn current_segment(&self) -> &TimeSeriesSegment {
        self.ts
            .segments
            .get(&self.current_time)
            .unwrap_or(&self.empty_segment)
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        let segment = self.current_segment();

        let title = Title::from("Status".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let text = vec![
            Line::from(format!(
                "Time:    {} ~ {} (between {} ~ {})",
                fmt_u64(segment.start_time.get() - self.base_time.get()),
                fmt_u64(segment.end_time.get().min(self.ts.end_time.get()) - self.base_time.get()),
                fmt_u64(self.ts.start_time.get() - self.base_time.get()),
                fmt_u64(self.ts.end_time.get() - self.base_time.get()),
            )),
            Line::from(format!(
                "Targets: {}",
                fmt_u64(segment.target_segment_values.len() as u64)
            )),
            Line::from(format!(
                "Items:   {} (filter={})",
                fmt_u64(segment.aggregated_values.len() as u64),
                self.options.item_filter
            )),
        ];
        Paragraph::new(text)
            .left_aligned()
            .block(block)
            .render(area, buf);
    }

    fn render_help(&self, area: Rect, buf: &mut Buffer) {
        let title = Title::from("Help".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);
        let main_layout = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]);
        let [header_area, _main_area] = main_layout.areas(area);

        let text = vec![Line::from(vec!["Quit: ".into(), "<Q>".blue().bold()])];
        Paragraph::new(text)
            .left_aligned()
            .block(block)
            .render(header_area, buf);
    }

    fn render_aggregation(&self, _area: Rect, _buf: &mut Buffer) {
        //
    }

    fn render_values(&self, _area: Rect, _buf: &mut Buffer) {}

    fn render_chart(&self, _area: Rect, _buf: &mut Buffer) {}
}

impl Widget for &ViewerApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (status_area, help_area, aggregation_area, values_area, chart_area) =
            self.calculate_layout(area);
        self.render_status(status_area, buf);
        self.render_help(help_area, buf);
        self.render_aggregation(aggregation_area, buf);
        self.render_values(values_area, buf);
        self.render_chart(chart_area, buf);
    }
}

fn fmt_u64(mut n: u64) -> String {
    if n == 0 {
        return n.to_string();
    }

    let mut s = Vec::new();
    let mut i = 0;
    while n > 0 {
        if i > 0 && i % 3 == 0 {
            s.push(',');
        }
        let d = (n % 10) as u8;
        s.push(char::from(b'0' + d));
        n /= 10;
        i += 1;
    }
    s.reverse();
    s.into_iter().collect()
}

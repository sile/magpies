use std::{collections::BTreeMap, fs::File, time::Duration};

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
    record::{FlattenedRecord, Record},
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct ViewerOptions {
    pub realtime: bool,
    pub interval: Duration,
    pub chart_time_window: Duration,
    pub item_filter: Regex,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct RecordKey {
    timestamp: Duration,
    seqno: u64,
}

impl RecordKey {
    fn new(record: &Record, next_seqno: &mut u64) -> Self {
        let seqno = *next_seqno;
        *next_seqno += 1;
        Self {
            timestamp: record.timestamp.to_duration(),
            seqno,
        }
    }
}

#[derive(Debug)]
pub struct Viewer {
    terminal: DefaultTerminal,
    options: ViewerOptions,
    reader: JsonlReader<File>,
    record_seqno: u64,
    exit: bool,
    app: ViewerApp,
}

impl Viewer {
    pub fn new(mut reader: JsonlReader<File>, options: ViewerOptions) -> orfail::Result<Self> {
        let mut record_seqno = 0;
        let mut records = BTreeMap::new();
        while let Some(record) = reader.read_item::<Record>().or_fail()? {
            records.insert(RecordKey::new(&record, &mut record_seqno), record.flatten());
        }

        let mut terminal = ratatui::init();
        terminal.clear().or_fail()?;

        Ok(Self {
            options: options.clone(),
            terminal,
            reader,
            record_seqno,
            exit: false,
            app: ViewerApp { records, options },
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

            if self.options.realtime {
                while let Some(record) = self.reader.read_item().or_fail()? {
                    self.app.records.insert(
                        RecordKey::new(&record, &mut self.record_seqno),
                        record.flatten(),
                    );
                    need_redraw = true;
                }
            }

            if need_redraw {
                self.draw().or_fail()?;
            }
        }

        Ok(())
    }

    fn draw(&mut self) -> orfail::Result<()> {
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
    records: BTreeMap<RecordKey, FlattenedRecord>,
}

impl ViewerApp {
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

    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        let title = if self.options.realtime {
            Title::from("Status (REALTIME)".bold())
        } else {
            Title::from("Status".bold())
        };
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let text = vec![
            Line::from("Time:    ... ~ ..."),
            Line::from("Targets: 3"),
            Line::from("Items:   5"),
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

    fn render_aggregation(&self, _area: Rect, _buf: &mut Buffer) {}

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

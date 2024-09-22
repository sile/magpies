use std::{fs::File, time::Duration};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use orfail::OrFail;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{block::Title, Block, Cell, Paragraph, Row, Table, TableState, Widget},
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
    pub decimal_places: u8,
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
        let mut need_redraw = false;
        if key.kind != KeyEventKind::Press {
            return Ok(need_redraw);
        }

        match key.code {
            KeyCode::Char('q') => {
                self.exit = true;
            }
            KeyCode::Char('p') => {
                self.app.go_to_prev_time();
                need_redraw = true;
            }
            KeyCode::Char('n') => {
                self.app.go_to_next_time();
                need_redraw = true;
            }
            KeyCode::Char('s') => {
                self.app.go_to_start_time();
                need_redraw = true;
            }
            KeyCode::Char('e') => {
                self.app.go_to_end_time();
                need_redraw = true;
            }
            _ => {}
        }

        Ok(need_redraw)
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
    tail: bool,
    agg_table_state: TableState,
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
            tail: false,
            agg_table_state: TableState::default().with_selected(0),
        }
    }

    fn insert_record(&mut self, record: &Record) {
        self.ts.insert(record);
    }

    fn go_to_prev_time(&mut self) {
        self.current_time = SecondsU64::new(
            self.current_time
                .get()
                .saturating_sub(self.options.interval.get()),
        )
        .max(self.ts.start_time);
        if self.ts.start_time != self.ts.last_start_time() {
            self.tail = false;
        }
    }

    fn go_to_next_time(&mut self) {
        self.current_time = SecondsU64::new(self.current_time.get() + self.options.interval.get())
            .min(self.ts.last_start_time());
        if self.current_time == self.ts.last_start_time() {
            self.tail = true;
        }
    }

    fn go_to_start_time(&mut self) {
        self.current_time = self.ts.start_time;
        if self.ts.start_time != self.ts.last_start_time() {
            self.tail = false;
        }
    }

    fn go_to_end_time(&mut self) {
        self.current_time = self.ts.last_start_time();
        self.tail = true;
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

        if self.tail {
            let prev_last_start_time = self
                .ts
                .last_start_time()
                .get()
                .checked_sub(self.options.interval.get());
            if Some(self.current_time.get()) == prev_last_start_time {
                self.current_time = self.ts.last_start_time();
            }
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
                "Time:    {}s ~ {}s (between {}s ~ {}s)",
                fmt_u64(segment.start_time.get() - self.base_time.get()),
                fmt_u64(
                    segment.end_time().get().min(self.ts.end_time.get()) - self.base_time.get()
                ),
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
        let text = vec![
            Line::from(vec!["Quit: ".into(), "<Q>".bold()]),
            Line::from(vec![
                "Time: ".into(),
                "<P>".bold(),
                "rev, ".into(),
                "<N>".bold(),
                "ext, ".into(),
                "<S>".bold(),
                "tart, ".into(),
                "<E>".bold(),
                "nd".into(),
            ]),
            Line::from(vec![
                "Move: ".into(),
                "<Left>, <Right>, <Up>, <Down>".bold(),
            ]),
        ];
        Paragraph::new(text)
            .left_aligned()
            .block(block)
            .render(area, buf);
    }

    fn render_aggregation(&self, area: Rect, buf: &mut Buffer) {
        let segment = self.current_segment();

        let title = Title::from("Aggregated Items".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let header = ["Name", "Sum", "Delta/s"]
            .into_iter()
            .map(|t| Cell::from(Text::from(t).centered()))
            .collect::<Row>()
            .style(Style::default().bold())
            .height(1);
        let rows = segment.aggregated_values.iter().map(|(name, agg_value)| {
            [
                Cell::from(Text::from(name.clone())),
                Cell::from(
                    Text::from(agg_value.sum_text(self.options.decimal_places)).right_aligned(),
                ),
                Cell::from(
                    Text::from(agg_value.delta_text(self.options.decimal_places)).right_aligned(),
                ),
            ]
            .into_iter()
            .collect::<Row>()
        });
        Table::new(
            rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .block(block)
        .render(area, buf);
    }

    fn render_values(&self, area: Rect, buf: &mut Buffer) {
        let title = Title::from("Values of ...".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);
        Paragraph::new(Text::from("TODO"))
            .left_aligned()
            .block(block)
            .render(area, buf);
    }

    fn render_chart(&self, area: Rect, buf: &mut Buffer) {
        let title = Title::from("Delta/s Chart of ...".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);
        Paragraph::new(Text::from("TODO"))
            .left_aligned()
            .block(block)
            .render(area, buf);
    }
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

// TODO
pub fn fmt_u64(mut n: u64) -> String {
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

pub fn fmt_i64(n: i64) -> String {
    if n < 0 {
        format!("-{}", fmt_u64(n.abs() as u64))
    } else {
        fmt_u64(n.abs() as u64)
    }
}

pub fn fmt_f64(n: f64, decimal_places: usize) -> String {
    let s = format!("{:.1$}", n, decimal_places);
    let mut iter = s.splitn(2, '.');
    let integer = iter.next().expect("unreachable");
    let fraction = iter.next().expect("unreachable");

    let mut s = Vec::new();
    for (i, c) in integer.chars().rev().enumerate() {
        if c != '-' && i > 0 && i % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }
    s.reverse();

    s.push('.');
    for (i, c) in fraction.chars().enumerate() {
        if i > 0 && i % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }

    s.into_iter().collect()
}

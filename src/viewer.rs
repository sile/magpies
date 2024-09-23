use std::{fs::File, time::Duration};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use orfail::OrFail;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Margin},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    symbols::{border, Marker},
    text::{Line, Text},
    widgets::{
        block::Title, Axis, Block, Cell, Chart, Dataset, GraphType, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Widget,
    },
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
    pub chart_marker: Marker,
}

#[derive(Debug)]
pub struct Viewer {
    terminal: DefaultTerminal,
    reader: JsonlReader<File>,
    exit: bool,
    app: ViewerApp,
    widget_state: ViewerWidgetState,
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
            widget_state: ViewerWidgetState::new(),
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

        self.widget_state.agg_table_scroll = self
            .widget_state
            .agg_table_scroll
            .content_length(self.app.current_segment().aggregated_values.len());

        self.widget_state.values_table_scroll = self
            .widget_state
            .values_table_scroll
            .content_length(self.app.current_segment().target_segment_values.len());

        self.terminal
            .draw(|frame| {
                frame.render_stateful_widget(&self.app, frame.area(), &mut self.widget_state)
            })
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
            KeyCode::Right => {
                self.app.in_agg_table = false;
                need_redraw = true;
            }
            KeyCode::Left => {
                self.app.in_agg_table = true;
                need_redraw = true;
            }
            KeyCode::Up => {
                self.move_cursor(-1);
                need_redraw = true;
            }
            KeyCode::Down => {
                self.move_cursor(1);
                need_redraw = true;
            }
            KeyCode::PageUp => {
                let height = if self.app.in_agg_table {
                    self.widget_state.agg_table_height
                } else {
                    self.widget_state.values_table_height
                };
                let n = height.saturating_sub(4).max(1) as i16;
                self.move_cursor(-n);
                need_redraw = true;
            }
            KeyCode::PageDown => {
                let height = if self.app.in_agg_table {
                    self.widget_state.agg_table_height
                } else {
                    self.widget_state.values_table_height
                };
                let n = height.saturating_sub(4).max(1) as i16;
                self.move_cursor(n);
                need_redraw = true;
            }
            _ => {}
        }

        Ok(need_redraw)
    }

    fn move_cursor(&mut self, delta: i16) {
        let (table, scroll) = if self.app.in_agg_table {
            (
                &mut self.widget_state.agg_table,
                &mut self.widget_state.agg_table_scroll,
            )
        } else {
            (
                &mut self.widget_state.values_table,
                &mut self.widget_state.values_table_scroll,
            )
        };
        if delta < 0 {
            table.scroll_up_by(delta.abs() as u16);
        } else {
            table.scroll_down_by(delta as u16);
        }
        *scroll = scroll
            .clone()
            .position(table.selected().unwrap_or_default());
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[derive(Debug)]
pub struct ViewerWidgetState {
    agg_table: TableState,
    agg_table_scroll: ScrollbarState,
    agg_table_height: u16,
    values_table: TableState,
    values_table_scroll: ScrollbarState,
    values_table_height: u16,
}

impl ViewerWidgetState {
    fn new() -> Self {
        Self {
            agg_table: TableState::default().with_selected(0),
            agg_table_scroll: ScrollbarState::new(0),
            agg_table_height: 0,
            values_table: TableState::default().with_selected(0),
            values_table_scroll: ScrollbarState::new(0),
            values_table_height: 0,
        }
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
    in_agg_table: bool,
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
            in_agg_table: true,
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
            self.current_time = self.ts.last_start_time();
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
                "<Left>, <Right>, <Up>, <Down>, <PageUp>, <PageDown>".bold(),
            ]),
        ];
        Paragraph::new(text)
            .left_aligned()
            .block(block)
            .render(area, buf);
    }

    fn render_aggregation(&self, area: Rect, buf: &mut Buffer, state: &mut ViewerWidgetState) {
        let segment = self.current_segment();

        let title = Title::from("Aggregated Items".bold());
        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let header = ["Name", "Value", "Delta/s"]
            .into_iter()
            .map(|t| Cell::from(Text::from(t).centered()))
            .collect::<Row>()
            .style(Style::default().bold())
            .height(1);
        let rows = segment.aggregated_values.iter().map(|(name, agg_value)| {
            [
                Cell::from(Text::from(name.as_str())),
                Cell::from(
                    Text::from(agg_value.sum_text(self.options.decimal_places)).right_aligned(),
                ),
                Cell::from(
                    Text::from(format!(
                        "{}  ", // "  " is the padding for scroll bar
                        agg_value.delta_text(self.options.decimal_places)
                    ))
                    .right_aligned(),
                ),
            ]
            .into_iter()
            .collect::<Row>()
        });
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .header(header)
        .column_spacing(1)
        .highlight_style(if self.in_agg_table {
            Style::new().reversed()
        } else {
            Style::new().bold()
        })
        .block(block);
        ratatui::widgets::StatefulWidget::render(table, area, buf, &mut state.agg_table);

        // Scrollbar
        ratatui::widgets::StatefulWidget::render(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            buf,
            &mut state.agg_table_scroll,
        );

        state.agg_table_height = area.height;
    }

    fn selected_item_key(&self, state: &ViewerWidgetState) -> Option<&str> {
        let segment = self.current_segment();
        state
            .agg_table
            .selected()
            .and_then(|i| segment.aggregated_values.keys().nth(i).map(|k| k.as_str()))
    }

    fn selected_target(&self, state: &ViewerWidgetState) -> Option<&str> {
        if self.in_agg_table {
            return None;
        }

        let segment = self.current_segment();
        state.values_table.selected().and_then(|i| {
            segment
                .target_segment_values
                .keys()
                .nth(i)
                .map(|k| k.as_str())
        })
    }

    fn render_values(&self, area: Rect, buf: &mut Buffer, state: &mut ViewerWidgetState) {
        let segment = self.current_segment();
        let key = self.selected_item_key(state);
        let title = if let Some(key) = key {
            Title::from(format!("Values of {key:?}").bold())
        } else {
            Title::from("Values".bold())
        };

        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let header = ["Target", "Value", "Delta/s"]
            .into_iter()
            .map(|t| Cell::from(Text::from(t).centered()))
            .collect::<Row>()
            .style(Style::default().bold())
            .height(1);
        let rows = key.iter().flat_map(|key| {
            segment
                .target_segment_values
                .iter()
                .filter_map(|(target, values)| {
                    values.get(*key).map(|value| {
                        [
                            Cell::from(Text::from(target.as_str())),
                            Cell::from(
                                Text::from(value.value_text(self.options.decimal_places))
                                    .right_aligned(),
                            ),
                            Cell::from(
                                Text::from(format!(
                                    "{}  ", // "  " is the padding for scroll bar
                                    value.delta_text(self.options.decimal_places)
                                ))
                                .right_aligned(),
                            ),
                        ]
                        .into_iter()
                        .collect::<Row>()
                    })
                })
        });
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .column_spacing(1)
        .highlight_style(if self.in_agg_table {
            Style::new()
        } else {
            Style::new().reversed()
        })
        .block(block);
        ratatui::widgets::StatefulWidget::render(table, area, buf, &mut state.values_table);

        // Scrollbar
        ratatui::widgets::StatefulWidget::render(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            buf,
            &mut state.values_table_scroll,
        );

        state.values_table_height = area.height;
    }

    fn render_chart(&self, area: Rect, buf: &mut Buffer, state: &ViewerWidgetState) {
        let key = self.selected_item_key(state);
        let target = self.selected_target(state);

        let title = if let Some(key) = key {
            Title::from(
                format!(
                    "Delta/s Chart of {key:?}{}",
                    if let Some(t) = target {
                        format!(" of {t:?}")
                    } else {
                        "".to_owned()
                    }
                )
                .bold(),
            )
        } else {
            Title::from("Delta/s Chart".bold())
        };

        let block = Block::bordered()
            .title(title.alignment(Alignment::Left))
            .border_set(border::THICK);

        let base_time = self.base_time.get();
        let end_time = self.current_time.get();
        let start_time = end_time
            .saturating_sub(
                self.options.interval.get() * self.options.chart_time_window.get()
                    / self.options.interval.get(),
            )
            .max(base_time);

        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        let mut data = Vec::new();
        for t in start_time..=end_time {
            let Some(segment) = self.ts.segments.get(&SecondsU64::new(t)) else {
                continue;
            };

            let delta = if self.in_agg_table {
                key.and_then(|k| segment.aggregated_values.get(k))
                    .and_then(|v| v.delta.as_ref().and_then(|v| v.as_f64()))
            } else {
                key.and_then(|k| {
                    target
                        .and_then(|t| {
                            segment
                                .target_segment_values
                                .get(t)
                                .and_then(|values| values.get(k))
                        })
                        .and_then(|v| v.delta.as_ref().and_then(|v| v.as_f64()))
                })
            };

            let Some(y) = delta else {
                continue;
            };
            data.push((t as f64, y));

            y_min = y_min.min(y);
            y_max = y_max.max(y);
        }
        if y_min.is_infinite() {
            y_min = -1.0;
            y_max = 1.0;
        }
        if y_min == y_max {
            let v = y_min;
            y_min = v - 1.0;
            y_max = v + 1.0;
        }

        let decimal_places = if y_min.fract() == 0.0 && y_max.fract() == 0.0 {
            0
        } else {
            self.options.decimal_places as usize
        };

        let datasets = vec![Dataset::default()
            .marker(self.options.chart_marker)
            .graph_type(GraphType::Line)
            .data(&data)];

        let chart = Chart::new(datasets)
            .x_axis(
                Axis::default()
                    .style(Style::default().gray())
                    .bounds([start_time as f64, end_time as f64])
                    .labels([
                        format!("{}s", fmt_u64(start_time - base_time)).bold(),
                        format!("{}s", fmt_u64(end_time - base_time)).bold(),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .style(Style::default().gray())
                    .bounds([y_min, y_max])
                    .labels([
                        fmt_f64(y_min, decimal_places).bold(),
                        fmt_f64(y_max, decimal_places).bold(),
                    ]),
            )
            .block(block);
        chart.render(area, buf);
    }
}

impl ratatui::widgets::StatefulWidget for &ViewerApp {
    type State = ViewerWidgetState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let (status_area, help_area, aggregation_area, values_area, chart_area) =
            self.calculate_layout(area);
        self.render_status(status_area, buf);
        self.render_help(help_area, buf);
        self.render_aggregation(aggregation_area, buf, state);
        self.render_values(values_area, buf, state);
        self.render_chart(chart_area, buf, state);
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
    let fraction = iter.next();

    let mut s = Vec::new();
    for (i, c) in integer.chars().rev().enumerate() {
        if c != '-' && i > 0 && i % 3 == 0 {
            s.push(',');
        }
        s.push(c);
    }
    s.reverse();

    if let Some(fraction) = fraction {
        s.push('.');
        for (i, c) in fraction.chars().enumerate() {
            if i > 0 && i % 3 == 0 {
                s.push(',');
            }
            s.push(c);
        }
    }

    s.into_iter().collect()
}

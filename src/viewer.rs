use std::{collections::BTreeMap, fs::File, time::Duration};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind};
use orfail::OrFail;
use ratatui::{style::Stylize, widgets::Paragraph, DefaultTerminal};

use crate::{jsonl::JsonlReader, record::Record};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub struct ViewerOptions {
    pub realtime: bool,
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
            timestamp: record.timestamp.get(),
            seqno,
        }
    }
}

#[derive(Debug)]
pub struct ViewerApp {
    options: ViewerOptions,
    terminal: DefaultTerminal,
    reader: JsonlReader<File>,
    records: BTreeMap<RecordKey, Record>,
    record_seqno: u64,
    quit: bool,
}

impl ViewerApp {
    pub fn new(mut reader: JsonlReader<File>, options: ViewerOptions) -> orfail::Result<Self> {
        let mut record_seqno = 0;
        let mut records = BTreeMap::new();
        while let Some(record) = reader.read_item::<Record>().or_fail()? {
            records.insert(RecordKey::new(&record, &mut record_seqno), record);
        }

        let mut terminal = ratatui::init();
        terminal.clear().or_fail()?;

        Ok(Self {
            options,
            terminal,
            reader,
            records,
            record_seqno,
            quit: false,
        })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        while !self.quit {
            self.terminal
                .draw(|frame| {
                    let greeting = Paragraph::new("Hello Ratatui! (press 'q' to quit)")
                        .white()
                        .on_blue();
                    frame.render_widget(greeting, frame.area());
                })
                .or_fail()?;

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
                    self.records
                        .insert(RecordKey::new(&record, &mut self.record_seqno), record);
                    need_redraw = true;
                }
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> orfail::Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }
        if key.code == KeyCode::Char('q') {
            self.quit = true;
        }
        Ok(false)
    }
}

impl Drop for ViewerApp {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

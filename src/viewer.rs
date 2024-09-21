use orfail::OrFail;

#[derive(Debug)]
pub struct ViewerOptions {
    pub realtime: bool,
}

#[derive(Debug)]
pub struct ViewerApp {
    options: ViewerOptions,
}

impl ViewerApp {
    pub fn new(options: ViewerOptions) -> orfail::Result<Self> {
        let mut terminal = ratatui::init();
        terminal.clear().or_fail()?;

        Ok(Self { options })
    }

    pub fn run(self) -> orfail::Result<Self> {
        todo!()
    }
}

impl Drop for ViewerApp {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

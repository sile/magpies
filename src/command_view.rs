#[derive(Debug, clap::Args)]
pub struct ViewCommand {}

impl ViewCommand {
    pub fn run(self) -> orfail::Result<()> {
        Ok(())
    }
}

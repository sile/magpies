use clap::Parser;
use magpies::command_poll::PollCommand;
use orfail::OrFail;

#[derive(Parser)]
enum Args {
    Poll(PollCommand),
}

fn main() -> orfail::Result<()> {
    let args = Args::parse();
    match args {
        Args::Poll(c) => c.run().or_fail()?,
    }
    Ok(())
}

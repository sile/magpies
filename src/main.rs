use clap::Parser;
use magpies::{command_poll::PollCommand, command_target::TargetCommand};
use orfail::OrFail;

#[derive(Parser)]
enum Args {
    Poll(PollCommand),
    Target(TargetCommand),
}

fn main() -> orfail::Result<()> {
    let args = Args::parse();
    match args {
        Args::Poll(c) => c.run().or_fail()?,
        Args::Target(c) => c.run().or_fail()?,
    }
    Ok(())
}

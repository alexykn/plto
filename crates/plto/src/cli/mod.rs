pub(crate) mod args;
pub(crate) mod handlers;
pub(crate) mod mapping;

use clap::Parser;

use self::args::Cli;

pub(crate) fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    handlers::run_command(cli.command)
}

use clap::Parser;

mod cli;
mod log;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    log::init(cli.options.verbose)?;
    cli::handle(cli)?;
    Ok(())
}

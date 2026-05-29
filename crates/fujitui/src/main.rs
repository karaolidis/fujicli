use clap::Parser;

mod library;
mod log;
mod tui;

fn main() -> anyhow::Result<()> {
    let cli = tui::Cli::parse();
    log::init(cli.verbose)?;
    tui::run()
}

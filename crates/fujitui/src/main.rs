use clap::Parser;

mod log;
mod tui;
mod ui;
mod workers;

fn main() -> anyhow::Result<()> {
    let cli = tui::Cli::parse();
    log::init(cli.verbose)?;
    tui::run()
}

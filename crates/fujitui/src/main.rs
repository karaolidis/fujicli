use anyhow::bail;
use clap::Parser;
use directories::ProjectDirs;

mod log;
mod tui;
mod ui;
mod workers;

fn main() -> anyhow::Result<()> {
    let cli = tui::Cli::parse();
    let Some(dirs) = ProjectDirs::from("", "", "fujicli") else {
        bail!("cannot determine project directories for this platform");
    };
    log::init(cli.verbose, &dirs)?;
    tui::run(&dirs)
}

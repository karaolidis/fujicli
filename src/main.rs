use clap::Parser;
mod cli;
mod log;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    log::init(cli.quiet, cli.verbose)?;

    match cli.command {
        _ => {}
    }

    Ok(())
}

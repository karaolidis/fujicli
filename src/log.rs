use log::LevelFilter;
use log4rs::{
    Config,
    append::console::{ConsoleAppender, Target},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

pub fn init(quiet: bool, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let level = if quiet {
        LevelFilter::Warn
    } else if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let encoder = Box::new(PatternEncoder::new("{d} {h({l})} {M}::{L} - {m}{n}"));

    let console = ConsoleAppender::builder()
        .encoder(encoder)
        .target(Target::Stderr)
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stderr", Box::new(console)))
        .build(Root::builder().appender("stderr").build(level))?;

    log4rs::init_config(config)?;

    Ok(())
}

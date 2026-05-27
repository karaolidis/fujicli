use log::LevelFilter;
use log4rs::{
    Config,
    append::console::{ConsoleAppender, Target},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

pub fn init(verbose: u8) -> anyhow::Result<()> {
    let level = match verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    #[allow(clippy::literal_string_with_formatting_args)]
    let pattern = if verbose > 0 {
        "{d} {h({l:5})} {M}::{L} - {m}{n}"
    } else {
        "{h({l:5})} - {m}{n}"
    };

    let encoder = Box::new(PatternEncoder::new(pattern));

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

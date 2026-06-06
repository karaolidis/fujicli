use anyhow::bail;
use directories::ProjectDirs;
use log::LevelFilter;
use log4rs::{
    Config,
    append::rolling_file::{
        RollingFileAppender,
        policy::compound::{
            CompoundPolicy, roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger,
        },
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

const ROLL_SIZE_BYTES: u64 = 5 * 1024 * 1024;
const ROLL_COUNT: u32 = 3;

pub fn init(verbose: u8, dirs: &ProjectDirs) -> anyhow::Result<()> {
    let level = match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let state_dir = dirs.state_dir().map_or_else(
        || dirs.data_dir().to_path_buf(),
        std::path::Path::to_path_buf,
    );
    std::fs::create_dir_all(&state_dir)?;

    let log_path = state_dir.join("fujitui.log");
    let rolled_pattern = state_dir.join("fujitui.{}.log");
    let Some(rolled_pattern_str) = rolled_pattern.to_str() else {
        bail!("log path is not valid UTF-8: {}", rolled_pattern.display());
    };

    let trigger = SizeTrigger::new(ROLL_SIZE_BYTES);
    let roller = FixedWindowRoller::builder().build(rolled_pattern_str, ROLL_COUNT)?;
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    #[allow(clippy::literal_string_with_formatting_args)]
    let encoder = PatternEncoder::new("{d} {l:5} {M}::{L} - {m}{n}");
    let appender = RollingFileAppender::builder()
        .encoder(Box::new(encoder))
        .build(&log_path, Box::new(policy))?;

    let config = Config::builder()
        .appender(Appender::builder().build("file", Box::new(appender)))
        .build(Root::builder().appender("file").build(level))?;

    log4rs::init_config(config)?;

    Ok(())
}

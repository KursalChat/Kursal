use crate::{KursalError, Result};
use anyhow::anyhow;
use log::LevelFilter;
use log4rs::{
    Config,
    append::{
        console::ConsoleAppender,
        rolling_file::{
            RollingFileAppender,
            policy::compound::{
                CompoundPolicy, roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger,
            },
        },
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

pub fn init_logging(log_level: &str, log_file: Option<&str>) -> Result<()> {
    let level: LevelFilter = log_level
        .parse()
        .map_err(|_| KursalError::Misc(anyhow!("Invalid log level: {log_level}")))?;

    let pattern = Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] {m}{n}"));

    let stdout = ConsoleAppender::builder().encoder(pattern.clone()).build();

    let mut config_builder =
        Config::builder().appender(Appender::builder().build("stdout", Box::new(stdout)));

    let root = match log_file {
        None => Root::builder().appender("stdout").build(level),
        Some(path) => {
            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| KursalError::Misc(anyhow!("Failed to create log dir: {err}")))?;
            }

            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|err| KursalError::Misc(anyhow!("Failed to create log file: {err}")))?;

            let roller = FixedWindowRoller::builder()
                .build(&format!("{path}.{{}}.gz"), 7)
                .map_err(|err| KursalError::Misc(anyhow!(err)))?;

            // Rotate when log file reaches 10 MB
            let trigger = SizeTrigger::new(10 * 1024 * 1024);
            let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

            let file = RollingFileAppender::builder()
                .encoder(pattern)
                .build(path, Box::new(policy))
                .map_err(|err| KursalError::Misc(anyhow!(err)))?;

            config_builder =
                config_builder.appender(Appender::builder().build("file", Box::new(file)));

            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(level)
        }
    };

    let config = config_builder
        .build(root)
        .map_err(|err| KursalError::Misc(anyhow!(err)))?;

    log4rs::init_config(config).map_err(|err| KursalError::Misc(anyhow!(err)))?;

    Ok(())
}

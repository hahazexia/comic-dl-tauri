use log::{debug, error, info, trace, warn, LevelFilter, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        rolling_file::policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};
// use std::time::Duration;
const TRIGGER_FILE_SIZE: u64 = 1024 * 1024;
// const TIME_BETWEEN_LOG_MESSAGES: Duration = Duration::from_millis(10);
const LOG_FILE_COUNT: u32 = 5;
// const RUN_TIME: Duration = Duration::from_secs(2);

pub fn init_log() -> Result<(), SetLoggerError> {
    let level = log::LevelFilter::Info;

    let stderr = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%+)(utc)} [{f}:{L}] {h({l})} {M}:{m}{n}",
        )))
        .target(Target::Stderr)
        .build();

    // let now = SystemTime::now()
    //     .duration_since(UNIX_EPOCH)
    //     .expect("Time went backwards");
    let timestamp = chrono::Local::now().format("%Y-%m-%d").to_string();

    let home_dir = home::home_dir().unwrap();
    let log_path = home_dir
        .join(".comic_dl_tauri")
        .join(format!("log/log-{}.log", timestamp));
    let archive_log_path = home_dir.join(".comic_dl_tauri").join("log/archive.{}.log");
    let file_path: &str = log_path.to_str().unwrap();
    let archive_path: &str = archive_log_path.to_str().unwrap();

    println!(
        "\n {} \n{} \n {}\n",
        &home_dir.to_str().unwrap(),
        &file_path,
        &archive_path
    );

    let roller = FixedWindowRoller::builder()
        .base(1)
        .build(format!("{}.{{}}", archive_path).as_str(), LOG_FILE_COUNT)
        .unwrap();
    let trigger = SizeTrigger::new(TRIGGER_FILE_SIZE); // 当文件大小达到 1MB 时滚动
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    // let trigger = SizeTrigger::new(TRIGGER_FILE_SIZE);
    // let roller = FixedWindowRoller::builder()
    //     .base(0) // Default Value (line not needed unless you want to change from 0 (only here for demo purposes)
    //     .build(archive_path, LOG_FILE_COUNT) // Roll based on pattern and max 3 archive files
    //     .unwrap();
    // let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    // Logging to log file. (with rolling)
    let logfile = log4rs::append::rolling_file::RollingFileAppender::builder()
        // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {l} {t} - {m}{n}",
        )))
        .build(file_path, Box::new(policy))
        .unwrap();

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level)))
                .build("logfile", Box::new(logfile)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level)))
                .build("stderr", Box::new(stderr)),
        )
        .build(
            Root::builder()
                .appender("logfile")
                .appender("stderr")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;

    error!("Goes to stderr and file");
    warn!("Goes to stderr and file");
    info!("Goes to stderr and file");
    debug!("Goes to file only");
    trace!("Goes to file only");

    // Generate some log messages to trigger rolling
    // let instant = Instant::now();
    // while instant.elapsed() < RUN_TIME {
    //     info!("Running for {:?}", instant.elapsed());
    //     sleep(TIME_BETWEEN_LOG_MESSAGES);
    // }
    info!(
        "See '{}' for log and '{}' for archived logs",
        file_path, archive_path
    );

    Ok(())
}

use core::str;

use log::{LevelFilter, Metadata, Record};

pub fn init(raw_level: &[u8; 16]) {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    // FIXME: Configure the logger
    let level = parse_level_filter(raw_level);
    log::set_max_level(level);

    info!("Logger Initialized with level {}.", level.as_str());
}

struct Logger;

fn parse_level_filter(raw_level: &[u8; 16]) -> LevelFilter {
    let len = raw_level
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(raw_level.len());

    let level = str::from_utf8(&raw_level[..len]).unwrap_or("info");

    if level.eq_ignore_ascii_case("off") {
        LevelFilter::Off
    } else if level.eq_ignore_ascii_case("error") {
        LevelFilter::Error
    } else if level.eq_ignore_ascii_case("warn") {
        LevelFilter::Warn
    } else if level.eq_ignore_ascii_case("debug") {
        LevelFilter::Debug
    } else if level.eq_ignore_ascii_case("trace") {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        // FIXME: Implement the logger with serial output
        if self.enabled(record.metadata()) {
            // 统一日志格式后交给 println!，最终会通过串口输出。
            println!(
                "[{}] {}:{} {}",
                record.level(),
                record.file_static().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

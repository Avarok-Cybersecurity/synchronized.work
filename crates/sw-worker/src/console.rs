//! `log` records to the isolate's console, filtered by the `LOG_LEVEL` var (required: an
//! unset level logs errors only and says so once).

use log::{Level, LevelFilter, Log, Metadata, Record};
use worker::Env;

pub const VAR_LOG_LEVEL: &str = "LOG_LEVEL";

struct Console;

impl Log for Console {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{}] {}: {}",
            record.level(),
            record.target(),
            record.args()
        );
        match record.level() {
            Level::Error => worker::console_error!("{line}"),
            Level::Warn => worker::console_warn!("{line}"),
            _ => worker::console_log!("{line}"),
        }
    }
    fn flush(&self) {}
}

static CONSOLE: Console = Console;

pub fn init(env: &Env) {
    if log::set_logger(&CONSOLE).is_err() {
        return;
    }
    let level = env.var(VAR_LOG_LEVEL).ok().map(|v| v.to_string());
    match level.as_deref().map(str::parse::<LevelFilter>) {
        Some(Ok(filter)) => log::set_max_level(filter),
        _ => {
            log::set_max_level(LevelFilter::Error);
            log::error!(target: "sw_worker", "{VAR_LOG_LEVEL} is unset or invalid; logging errors only");
        }
    }
}

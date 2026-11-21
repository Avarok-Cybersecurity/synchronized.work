//! The `log` facade to `log/sw-mcp.log` (owner-only) and stderr (never stdout: `serve` speaks MCP
//! there). Kids appear as 8-hex prefixes and prompt text never does (design §6.8).

use std::fs::OpenOptions;
use std::io::Write as _;
use std::sync::Mutex;

use crate::config::LogLevel;
use crate::home::Home;

struct Logger {
    level: log::LevelFilter,
    file: Mutex<std::fs::File>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level && metadata.target().starts_with("sw_mcp")
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:5} {}\n",
            crate::daemon::now_ms(),
            record.level(),
            record.args()
        );
        eprint!("sw-mcp: {line}");
        if let Ok(mut f) = self.file.lock()
            && let Err(e) = f.write_all(line.as_bytes())
        {
            eprintln!("sw-mcp: could not write the log file: {e}");
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

pub fn init(home: &Home, level: LogLevel) -> Result<(), String> {
    let level = match level {
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
    };
    let file = crate::home::open_private(
        OpenOptions::new().create(true).append(true),
        &home.log_file(),
    )
    .map_err(|e| format!("{}: {e}", home.log_file().display()))?;
    log::set_boxed_logger(Box::new(Logger {
        level,
        file: Mutex::new(file),
    }))
    .map_err(|e| e.to_string())?;
    log::set_max_level(level);
    Ok(())
}

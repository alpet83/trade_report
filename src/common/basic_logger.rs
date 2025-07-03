use chrono::{DateTime, Utc, Timelike};
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
// use std::path::Path;
use std::sync::{Arc, Mutex};
// use std::thread;
// use std::time::Duration;
use tracing::{debug, error, info, warn};
use regex::Regex;
use backtrace::Backtrace;

// Global log directory, can be overridden in tests
static DEFAULT_LOG_DIR: OnceCell<Arc<String>> = OnceCell::new();
const SIZE_LIMIT: u32 = 300 * 1024 * 1024; // 300 MB

// Singleton for storing loggers per thread
static LOGGER_MAP: OnceCell<Arc<DashMap<u32, Arc<Mutex<BasicLogger>>>>> = OnceCell::new();

// Gets or creates the global logger map
fn logger_map() -> Arc<DashMap<u32, Arc<Mutex<BasicLogger>>>> {
    LOGGER_MAP
        .get_or_init(|| Arc::new(DashMap::new()))
        .clone()
}

// Gets the log directory, defaulting to "logs"
fn get_log_dir() -> Arc<String> {
    DEFAULT_LOG_DIR
        .get_or_init(|| Arc::new("logs".to_string()))
        .clone()
}


#[derive(Debug)]
pub struct BasicLogger {
    log_prefix: String,
    log_dir: String,
    file_name: String,
    file: Option<File>,
    console_out: bool,
    use_color_in_files: bool,
    lines: u32,
    lines_limit: u32,
    size_limit: u32,
    last_create: DateTime<Utc>,
    pub thread_id: u32,
}

impl BasicLogger {
    pub fn new(log_prefix: &str, thread_id: u32, console_out: bool, use_color_in_files: bool) -> Self {
        let log_dir = get_log_dir();
        let file_name = format!("{}/{}{}.log", log_dir, log_prefix, thread_id);
        debug!("#DBG: Creating logger for thread_id={}, file_name={}", thread_id, file_name);
        if let Err(e) = fs::create_dir_all(&*log_dir) {
            error!("#ERROR: Failed to create log directory {}: {}", log_dir, e);
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&file_name)
            .map_err(|e| error!("#ERROR: Failed to open log file {}: {}", file_name, e))
            .ok();
        if file.is_some() {
            debug!("#DBG: Successfully opened log file {}", file_name);
        } else {
            error!("#ERROR: Failed to create or open log file {}", file_name);
        }
        BasicLogger {
            log_prefix: log_prefix.to_string(),
            log_dir: log_dir.to_string(),
            file_name,
            file,
            console_out,
            use_color_in_files,
            lines: 0,
            lines_limit: 15_000,
            size_limit: SIZE_LIMIT,
            last_create: Utc::now(),
            thread_id,
        }
    }

    fn file_size(&self) -> u64 {
        self.file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .map(|m| m.len())
            .unwrap_or_else(|| {
                fs::metadata(&self.file_name)
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
    }

    fn rotate(&mut self) -> io::Result<()> {
        debug!("#DBG: Rotating log file {}", self.file_name);
        if let Some(mut f) = self.file.take() {
            f.flush()?;
            debug!("#DBG: Flushed file {}", self.file_name);
        }
        let timestamp = self.last_create.format("%Y-%m-%d_%H-%M-%S").to_string();
        let base_name = self.file_name.strip_suffix(".log").unwrap_or(&self.file_name);
        let new_name = format!("{}_{}.log", base_name, timestamp);
        if let Err(e) = fs::rename(&self.file_name, &new_name) {
            warn!("#WARN: Failed to rename log {} to {}: {}", self.file_name, new_name, e);
        } else {
            info!("#INFO: Renamed log to {}", new_name);
        }
        self.file_name = format!("{}/{}{}.log", self.log_dir, self.log_prefix, self.thread_id);
        debug!("#DBG: New log file name: {}", self.file_name);
        self.file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&self.file_name)
            .map_err(|e| error!("#ERROR: Failed to open new log file {}: {}", self.file_name, e))
            .ok();
        if self.file.is_some() {
            debug!("#DBG: Successfully opened new log file {}", self.file_name);
        } else {
            error!("#ERROR: Failed to create or open new log file {}", self.file_name);
        }
        self.lines = 0;
        self.last_create = Utc::now();
        Ok(())
    }

    fn write_log(&mut self, level: &str, msg: &str) -> io::Result<()> {
        let timestamp = Utc::now().format("[%y-%m-%d %H:%M:%S%.3f]").to_string();
        let log_msg = if self.use_color_in_files {
            let colored_msg = colorize_msg(msg);
            format!("{} [{}] {}\n", timestamp, level, colored_msg)
        } else {
            let stripped_msg = strip_color(msg);
            format!("{} [{}] {}\n", timestamp, level, stripped_msg)
        };

        if let Some(ref mut f) = self.file {
            f.write_all(log_msg.as_bytes())?;
            f.flush()?;
        } else {
            error!("#ERROR: No file handle for {}, cannot write log", self.file_name);
            return Err(io::Error::new(io::ErrorKind::Other, format!("No file handle for {}", self.file_name)));
        }

        self.lines += msg.matches('\n').count() as u32 + 1;
        if self.lines >= self.lines_limit || self.file_size() > self.size_limit as u64 {
            debug!("#DBG: Triggering rotation for {}", self.file_name);
            self.rotate()?;
        }

        if self.console_out {
            let colored_msg = colorize_msg(msg);
            match level {
                "INFO" => info!("{}", colored_msg),
                "WARN" => warn!("{}", colored_msg),
                "DEBUG" => debug!("{}", colored_msg),
                "ERROR" => error!("{}", colored_msg),
                _ => info!("{}", colored_msg),
            }
        }

        Ok(())
    }

    pub fn info(&mut self, msg: &str) -> io::Result<()> {
        self.write_log("INFO", msg)
    }

    pub fn warn(&mut self, msg: &str) -> io::Result<()> {
        self.write_log("WARN", msg)
    }

    pub fn debug(&mut self, msg: &str) -> io::Result<()> {
        self.write_log("DEBUG", msg)
    }

    pub fn error(&mut self, msg: &str) -> io::Result<()> {
        let backtrace = filtered_backtrace();
        let full_msg = format!("{}\nBacktrace:\n{}", msg, backtrace);
        self.write_log("ERROR", &full_msg)
    }

    pub fn close(&mut self) -> io::Result<()> {
        debug!("#DBG: Closing logger for {}", self.file_name);
        if let Some(mut f) = self.file.take() {
            f.flush()?;
            debug!("#DBG: Flushed and closed file {}", self.file_name);
        }
        Ok(())
    }

    pub fn set_limit_lines(&mut self, limit: u32) {
        debug!("#DBG: Setting lines limit for {} to {}", self.file_name, limit);
        self.lines_limit = limit;
    }
}

impl Drop for BasicLogger {
    fn drop(&mut self) {
        debug!("#DBG: Dropping logger for {}", self.file_name);
        if let Err(e) = self.close() {
            error!("#ERROR: Failed to close logger: {}", e);
        }
    }
}

// Filters backtrace to include only project-specific frames
fn filtered_backtrace() -> String {
    let backtrace = Backtrace::new();
    let mut result = String::new();
    for frame in backtrace.frames() {
        for symbol in frame.symbols() {
            if let Some(file) = symbol.filename() {
                let file_path = file.to_string_lossy();
                if file_path.contains("src/") && !file_path.contains("target/") {
                    let symbol_name = symbol.name().map(|s| s.to_string()).unwrap_or_default();
                    let line = symbol.lineno().map(|l| l.to_string()).unwrap_or_default();
                    result.push_str(&format!("  at {}:{} in {}\n", file_path, line, symbol_name));
                }
            }
        }
    }
    if result.is_empty() {
        result = "  No project-specific frames found\n".to_string();
    }
    result
}

// Colorizes a message with ANSI escape sequences
fn colorize_msg(msg: &str) -> String {
    let re_color = Regex::new(r"~C(\d\d|38#[\da-fA-F]{6}|48#[\da-fA-F]{6})").unwrap();
    let mut result = msg.to_string();

    for cap in re_color.captures_iter(msg) {
        let code = cap.get(1).unwrap().as_str();
        let ansi_code: String = match code {
            "91" => "\x1B[91m".to_string(), // Red
            "92" => "\x1B[92m".to_string(), // Green
            "93" => "\x1B[93m".to_string(), // Yellow
            "94" => "\x1B[94m".to_string(), // Blue
            "95" => "\x1B[95m".to_string(), // Magenta
            "96" => "\x1B[96m".to_string(), // Cyan
            "97" => "\x1B[97m".to_string(), // White
            code if code.starts_with("38#") => {
                let rgb = &code[3..];
                let r = u8::from_str_radix(&rgb[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&rgb[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&rgb[4..6], 16).unwrap_or(0);
                format!("\x1B[38;2;{};{};{}m", r, g, b)
            }
            code if code.starts_with("48#") => {
                let rgb = &code[3..];
                let r = u8::from_str_radix(&rgb[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&rgb[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&rgb[4..6], 16).unwrap_or(0);
                format!("\x1B[48;2;{};{};{}m", r, g, b)
            }
            _ => "".to_string(),
        };
        result = result.replace(&format!("~C{}", code), ansi_code.as_str());
    }

    if re_color.is_match(msg) {
        result.push_str("\x1B[0m");
    }
    result
}

// Removes color tags and ANSI sequences for test validation
fn strip_color(msg: &str) -> String {
    let re = Regex::new(r"~C(\d\d|38#[\da-fA-F]{6}|48#[\da-fA-F]{6})|\x1B\[(?:\d+(?:;\d+)*)?m").unwrap();
    re.replace_all(msg, "").to_string()
}

pub mod logger {
    use super::{logger_map, BasicLogger, filtered_backtrace, DEFAULT_LOG_DIR};
    use std::sync::{Arc, Mutex};
    use std::io;
    use tracing::{debug, warn};
    use std::time::Duration;
    use std::thread;

    // Gets the executable name for log prefix
    fn get_exe_prefix() -> String {
        std::env::current_exe()
            .map(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|s| format!("{}#", s))
                    .unwrap_or_else(|| "trade_report#".to_string())
            })
            .unwrap_or_else(|_| "trade_report#".to_string())
    }

    // Gets or creates a logger for the current thread
    pub fn get() -> Arc<Mutex<BasicLogger>> {
        let thread_id = std::thread::current().id();
        let thread_id_str = format!("{:?}", thread_id);
        let thread_id_num = thread_id_str
            .strip_prefix("ThreadId(")
            .and_then(|s| s.strip_suffix(")"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let map = logger_map();
        let result = map.entry(thread_id_num)
            .or_insert_with(|| {
                debug!("#DBG: Creating new logger for thread_id={}", thread_id_num);
                Arc::new(Mutex::new(BasicLogger::new(
                    &get_exe_prefix(),
                    thread_id_num,
                    true,
                    true,
                )))
            })
            .clone();
        result
    }

    // Returns a vector of all thread IDs in the logger map
    pub fn get_ids() -> Vec<u32> {
        let map = logger_map();
        let ids: Vec<u32> = map.iter().map(|entry| *entry.key()).collect();
        debug!("#DBG: Retrieved {} logger IDs: {:?}", ids.len(), ids);
        ids
    }

    pub fn try_log(level: &str, msg: &str) -> io::Result<()> {
        let max_attempts = 3;
        let thread_id = std::thread::current().id();
        for attempt in 1..=max_attempts {
            match get().try_lock() {
                Ok(mut logger) => return logger.write_log(level, msg),
                Err(e) => {
                    warn!("#SYNC_FAIL(write_log/{}): Can't acquire lock for logger (attempt {} of {}): {} in {:?}", level, attempt, max_attempts, e, thread_id);
                    if attempt < max_attempts {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
        let error_msg = format!("Failed to acquire lock after {} attempts", max_attempts);
        let backtrace = filtered_backtrace();
        let full_msg = format!("{}\nBacktrace:\n{}", error_msg, backtrace);
        Err(io::Error::new(io::ErrorKind::Other, full_msg))
    }

    pub fn info(msg: &str) -> io::Result<()> {
        try_log("INFO", msg)
    }

    pub fn warn(msg: &str) -> io::Result<()> {
        try_log("WARN", msg)
    }

    pub fn debug(msg: &str) -> io::Result<()> {
        try_log("DEBUG", msg)
    }

    pub fn error(msg: &str) -> io::Result<()> {
        let backtrace = filtered_backtrace();
        let full_msg = format!("{}\nBacktrace:\n{}", msg, backtrace);
        try_log("ERROR", &full_msg)
    }

    pub fn set_log_dir(log_dir: &str) {
        DEFAULT_LOG_DIR.set(Arc::new(log_dir.to_string())).expect("Failed to set DEFAULT_LOG_DIR");
    }   
}
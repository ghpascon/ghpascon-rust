use chrono::{Duration, Local, NaiveDate};
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, OnceLock, Weak};
use tokio::{fs, time};

// ── Global instance ───────────────────────────────────────────────────────────

static GLOBAL: OnceLock<Arc<LoggerManager>> = OnceLock::new();

/// Log severity levels in increasing priority order.
///
/// Only messages at or above the configured level are recorded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    app: String,
    message: String,
    module: String,
    file: String,
    line: u32,
    thread: String,
}

enum LogMessage {
    Entry(LogEntry),
    Shutdown,
}

/// Non-blocking JSON logger with daily rotation and automatic old-file cleanup.
///
/// # Behaviour
///
/// * Log files are named `{YYYY-MM-DD}_{app_name}.jsonl` and written under `log_path`.
/// * A background task cleans files older than `retention_days` once per day.
/// * Logging is **non-blocking**: entries use a buffered channel (`try_send`).
///   If the buffer (4 096 entries) is full the entry is silently discarded.
/// * Uncaught panics are captured via `std::panic::set_hook` and written as
///   `"PANIC"` entries before the process unwinds.
///
/// # Example
///
/// ```no_run
/// use ghpascon_rust::utils::logger_manager::{LoggerManager, LogLevel};
///
/// #[tokio::main]
/// async fn main() {
///     let logger = LoggerManager::builder("myapp")
///         .level(LogLevel::Warn)
///         .log_path("var/log")
///         .retention_days(30)
///         .build()
///         .await;
///
///     logger.warn("high memory usage");
///     logger.error("connection refused");
/// }
/// ```
pub struct LoggerManager {
    app_name: String,
    log_path: PathBuf,
    retention_days: i64,
    level: LogLevel,
    sender: std_mpsc::SyncSender<LogMessage>,
    writer_thread: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Builder for [`LoggerManager`]. Obtain one via [`LoggerManager::builder`].
pub struct LoggerManagerBuilder {
    app_name: String,
    level: LogLevel,
    log_path: PathBuf,
    retention_days: i64,
}

impl LoggerManagerBuilder {
    /// Sets the minimum log level (default: [`LogLevel::Info`]).
    pub fn level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Sets the directory where log files are stored (default: `"logs"`).
    pub fn log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_path = path.into();
        self
    }

    /// Sets how many days to keep log files before deletion (default: `7`).
    pub fn retention_days(mut self, days: i64) -> Self {
        self.retention_days = days;
        self
    }

    /// Starts the logger and returns an `Arc<LoggerManager>`.
    pub async fn build(self) -> Arc<LoggerManager> {
        if let Err(e) = fs::create_dir_all(&self.log_path).await {
            eprintln!(
                "[LoggerManager] Failed to create log directory {:?}: {}",
                self.log_path, e
            );
        }

        let (sender, receiver) = std_mpsc::sync_channel::<LogMessage>(4096);
        let (log_path, app_name) = (self.log_path.clone(), self.app_name.clone());

        let manager = Arc::new(LoggerManager {
            app_name: self.app_name,
            log_path: self.log_path,
            retention_days: self.retention_days,
            level: self.level,
            sender,
            writer_thread: std::sync::Mutex::new(None),
        });

        *manager.writer_thread.lock().unwrap() = Some(std::thread::spawn(move || {
            LoggerManager::writer_fn(receiver, log_path, app_name)
        }));

        manager.cleanup_old_logs().await;

        let weak: Weak<LoggerManager> = Arc::downgrade(&manager);
        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_secs(86_400));
            interval.tick().await;
            loop {
                interval.tick().await;
                match weak.upgrade() {
                    Some(m) => m.cleanup_old_logs().await,
                    None => break,
                }
            }
        });

        let panic_sender = manager.sender.clone();
        let panic_app = manager.app_name.clone();
        std::panic::set_hook(Box::new(move |info| {
            let loc = info
                .location()
                .map(|l| format!(" at {}:{}", l.file(), l.line()))
                .unwrap_or_default();
            let payload = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            let _ = panic_sender.try_send(LogMessage::Entry(LogEntry {
                timestamp: Local::now().to_rfc3339(),
                level: "PANIC".to_string(),
                app: panic_app.clone(),
                message: format!("panic{}: {}", loc, payload),
                module: String::new(),
                file: String::new(),
                line: 0,
                thread: std::thread::current()
                    .name()
                    .unwrap_or("unnamed")
                    .to_string(),
            }));
            std::thread::sleep(std::time::Duration::from_millis(200));
        }));

        manager._log_internal(
            LogLevel::Info,
            format!(
                "LoggerManager started | app={} path={:?} retain={}d level={}",
                manager.app_name, manager.log_path, manager.retention_days, manager.level
            ),
            module_path!(),
            file!(),
            line!(),
        );

        manager
    }
}

impl LoggerManager {
    /// Returns a builder for constructing a `LoggerManager`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ghpascon_rust::utils::logger_manager::{LoggerManager, LogLevel};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let logger = LoggerManager::builder("myapp")
    ///         .level(LogLevel::Debug)
    ///         .build()
    ///         .await;
    /// }
    /// ```
    pub fn builder(app_name: impl Into<String>) -> LoggerManagerBuilder {
        LoggerManagerBuilder {
            app_name: app_name.into(),
            level: LogLevel::Info,
            log_path: PathBuf::from("logs"),
            retention_days: 7,
        }
    }

    // ── Global logger ─────────────────────────────────────────────────────────

    /// Registers this `LoggerManager` as the process-wide global instance.
    ///
    /// After calling this, the `println!`, `eprintln!`, `print!`, and `eprint!`
    /// macros exported by this crate will route their output here as `DEBUG`
    /// entries instead of writing to stdout/stderr.
    ///
    /// Can only be set once; subsequent calls are silently ignored.
    pub fn set_as_global(self: &Arc<Self>) {
        let _ = GLOBAL.set(Arc::clone(self));
    }

    /// Returns the global `LoggerManager`, if one has been registered via
    /// [`set_as_global`](Self::set_as_global).
    pub fn global() -> Option<Arc<LoggerManager>> {
        GLOBAL.get().cloned()
    }

    // Public logging API

    /// Internal method used by the `lgr_*!` macros for full call-site metadata.
    #[doc(hidden)]
    pub fn _log_internal(
        &self,
        level: LogLevel,
        message: String,
        module: &str,
        file: &str,
        line: u32,
    ) {
        if level < self.level {
            return;
        }
        let _ = self.sender.try_send(LogMessage::Entry(LogEntry {
            timestamp: Local::now().to_rfc3339(),
            level: level.as_str().to_string(),
            app: self.app_name.clone(),
            message,
            module: module.to_string(),
            file: file.to_string(),
            line,
            thread: std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string(),
        }));
    }

    /// Emits a log entry at the given level. Non-blocking.
    /// Use the `lgr_*!` macros for module/file metadata.
    #[track_caller]
    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        let loc = std::panic::Location::caller();
        self._log_internal(level, message.into(), "", loc.file(), loc.line());
    }

    #[track_caller]
    pub fn debug(&self, msg: impl Into<String>) {
        self.log(LogLevel::Debug, msg);
    }
    #[track_caller]
    pub fn info(&self, msg: impl Into<String>) {
        self.log(LogLevel::Info, msg);
    }
    #[track_caller]
    pub fn warn(&self, msg: impl Into<String>) {
        self.log(LogLevel::Warn, msg);
    }
    #[track_caller]
    pub fn error(&self, msg: impl Into<String>) {
        self.log(LogLevel::Error, msg);
    }

    // ── Background tasks ──────────────────────────────────────────────────────

    fn writer_fn(receiver: std_mpsc::Receiver<LogMessage>, log_path: PathBuf, app_name: String) {
        if let Err(e) = std::fs::create_dir_all(&log_path) {
            eprintln!(
                "[LoggerManager] Cannot create log dir {:?}: {}",
                log_path, e
            );
        }
        while let Ok(msg) = receiver.recv() {
            let LogMessage::Entry(entry) = msg else { break };

            // ── console ──────────────────────────────────────────────────────
            let (pad, color) = match entry.level.as_str() {
                "DEBUG" => ("DEBUG", "\x1b[90m"),
                "INFO" => ("INFO ", "\x1b[32m"),
                "WARN" => ("WARN ", "\x1b[33m"),
                "ERROR" => ("ERROR", "\x1b[31m"),
                "PANIC" => ("PANIC", "\x1b[1;31m"),
                other => (other, ""),
            };
            let loc = if !entry.file.is_empty() && entry.line > 0 {
                format!("  \x1b[2m{}:{}\x1b[0m", entry.file, entry.line)
            } else if !entry.module.is_empty() {
                format!("  \x1b[2m{}\x1b[0m", entry.module)
            } else {
                String::new()
            };
            println!(
                "\x1b[2m{}\x1b[0m  {}{}\x1b[0m  \x1b[1m{}\x1b[0m{}  {}",
                Local::now().format("%H:%M:%S"),
                color,
                pad,
                entry.app,
                loc,
                entry.message,
            );

            // ── file ─────────────────────────────────────────────────────────
            let file_path = log_path.join(format!(
                "{}_{}.jsonl",
                Local::now().format("%Y-%m-%d"),
                app_name
            ));
            if let Ok(json) = serde_json::to_string(&entry) {
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file_path)
                {
                    Ok(mut f) => {
                        if let Err(e) = writeln!(f, "{}", json) {
                            eprintln!("[LoggerManager] Write error: {}", e);
                        }
                    }
                    Err(e) => eprintln!("[LoggerManager] Cannot open {:?}: {}", file_path, e),
                }
            }
        }
    }

    async fn cleanup_old_logs(&self) {
        let cutoff = (Local::now() - Duration::days(self.retention_days)).date_naive();
        let mut entries = match fs::read_dir(&self.log_path).await {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[LoggerManager] Cannot read log dir: {}", e);
                return;
            }
        };
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let is_old = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|n| n.get(..10))
                        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                        .map_or(false, |d| d < cutoff);
                    if is_old {
                        if let Err(e) = fs::remove_file(&path).await {
                            eprintln!("[LoggerManager] Failed to delete {:?}: {}", path, e);
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("[LoggerManager] Dir read error: {}", e);
                    break;
                }
            }
        }
    }
}

// ── Macros ───────────────────────────────────────────────────────────────────

/// Logs at DEBUG level with full call-site metadata.
#[macro_export]
macro_rules! lgr_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger._log_internal(
            $crate::utils::logger_manager::LogLevel::Debug,
            format!($($arg)*), module_path!(), file!(), line!(),
        )
    };
}

/// Logs at INFO level with full call-site metadata.
#[macro_export]
macro_rules! lgr_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger._log_internal(
            $crate::utils::logger_manager::LogLevel::Info,
            format!($($arg)*), module_path!(), file!(), line!(),
        )
    };
}

/// Logs at WARN level with full call-site metadata.
#[macro_export]
macro_rules! lgr_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger._log_internal(
            $crate::utils::logger_manager::LogLevel::Warn,
            format!($($arg)*), module_path!(), file!(), line!(),
        )
    };
}

/// Logs at ERROR level with full call-site metadata.
#[macro_export]
macro_rules! lgr_error {
    ($logger:expr, $($arg:tt)*) => {
        $logger._log_internal(
            $crate::utils::logger_manager::LogLevel::Error,
            format!($($arg)*), module_path!(), file!(), line!(),
        )
    };
}

// ── println! / eprintln! / print! / eprint! overrides ────────────────────────
//
// Import these to redirect your prints to the global logger as DEBUG entries:
//
//   use ghpascon_rust::{println, eprintln};   // shadows the std macros
//
// If no global logger is set the output falls back to the standard streams.

/// Like `println!` but routes to the global [`LoggerManager`] at DEBUG level.
/// Falls back to `std::println!` when no global logger is registered.
#[macro_export]
macro_rules! println {
    () => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(""),
            None => ::std::println!(),
        }
    };
    ($($arg:tt)*) => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(::std::format!($($arg)*)),
            None => ::std::println!($($arg)*),
        }
    };
}

/// Like `eprintln!` but routes to the global [`LoggerManager`] at DEBUG level.
/// Falls back to `std::eprintln!` when no global logger is registered.
#[macro_export]
macro_rules! eprintln {
    () => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(""),
            None => ::std::eprintln!(),
        }
    };
    ($($arg:tt)*) => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(::std::format!($($arg)*)),
            None => ::std::eprintln!($($arg)*),
        }
    };
}

/// Like `print!` but routes to the global [`LoggerManager`] at DEBUG level.
/// Falls back to `std::print!` when no global logger is registered.
#[macro_export]
macro_rules! print {
    () => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(""),
            None => ::std::print!(),
        }
    };
    ($($arg:tt)*) => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(::std::format!($($arg)*)),
            None => ::std::print!($($arg)*),
        }
    };
}

/// Like `eprint!` but routes to the global [`LoggerManager`] at DEBUG level.
/// Falls back to `std::eprint!` when no global logger is registered.
#[macro_export]
macro_rules! eprint {
    () => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(""),
            None => ::std::eprint!(),
        }
    };
    ($($arg:tt)*) => {
        match $crate::utils::logger_manager::LoggerManager::global() {
            Some(lgr) => lgr.debug(::std::format!($($arg)*)),
            None => ::std::eprint!($($arg)*),
        }
    };
}

impl Drop for LoggerManager {
    fn drop(&mut self) {
        let _ = self.sender.send(LogMessage::Shutdown);
        if let Ok(mut guard) = self.writer_thread.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_creates_log_file() {
        let dir = tempdir().unwrap();
        let logger = LoggerManager::builder("testapp")
            .level(LogLevel::Debug)
            .log_path(dir.path())
            .retention_days(7)
            .build()
            .await;

        logger.info("hello from test");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let date = Local::now().format("%Y-%m-%d");
        let log_file = dir.path().join(format!("{}_testapp.jsonl", date));
        assert!(log_file.exists(), "log file should have been created");

        let content = std::fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("\"level\":\"INFO\""));
        assert!(content.contains("hello from test"));
    }

    #[tokio::test]
    async fn test_level_filtering() {
        let dir = tempdir().unwrap();
        let logger = LoggerManager::builder("filterapp")
            .level(LogLevel::Warn)
            .log_path(dir.path())
            .retention_days(7)
            .build()
            .await;

        logger.debug("ignored debug");
        logger.info("ignored info");
        logger.warn("visible warn");
        logger.error("visible error");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let date = Local::now().format("%Y-%m-%d");
        let log_file = dir.path().join(format!("{}_filterapp.jsonl", date));
        let content = std::fs::read_to_string(&log_file).unwrap();
        assert!(!content.contains("ignored"));
        assert!(content.contains("visible warn"));
        assert!(content.contains("visible error"));
    }

    #[tokio::test]
    async fn test_creates_directory_if_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("deep/nested/logs");
        let logger = LoggerManager::builder("nestapp")
            .log_path(nested.clone())
            .build()
            .await;
        logger.info("test");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(nested.exists());
    }

    #[tokio::test]
    async fn test_json_format() {
        let dir = tempdir().unwrap();
        let logger = LoggerManager::builder("jsonapp")
            .level(LogLevel::Debug)
            .log_path(dir.path())
            .build()
            .await;
        logger.error("boom");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let date = Local::now().format("%Y-%m-%d");
        let log_file = dir.path().join(format!("{}_jsonapp.jsonl", date));
        let content = std::fs::read_to_string(&log_file).unwrap();
        let line = content.lines().find(|l| l.contains("boom")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["level"], "ERROR");
        assert_eq!(parsed["app"], "jsonapp");
        assert_eq!(parsed["message"], "boom");
        assert!(parsed["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_old_logs_are_deleted() {
        let dir = tempdir().unwrap();
        // Nome com data antiga — o cleanup parseia a data do nome do arquivo
        let old_file = dir.path().join("2000-01-01_oldapp.jsonl");
        std::fs::write(&old_file, "old\n").unwrap();

        let logger = LoggerManager::builder("oldapp")
            .log_path(dir.path())
            .retention_days(7)
            .build()
            .await;
        // Cleanup runs immediately on first interval tick; give it time
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        assert!(!old_file.exists(), "old log file should have been deleted");
        drop(logger);
    }
}

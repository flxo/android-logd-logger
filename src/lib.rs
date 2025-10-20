//! A pure Rust logger for Android's `logd` logging system.
//!
//! This crate provides a logger implementation that writes directly to Android's `logd` socket,
//! bypassing the need for `liblog` or any other native Android libraries. On non-Android platforms,
//! logs are printed to stderr in a format similar to `logcat`.
//!
//! # Features
//!
//! - **Pure Rust**: No FFI or native dependencies required
//! - **Direct socket communication**: Writes directly to the `logd` socket
//! - **Multiple log buffers**: Support for main, radio, events, system, crash, stats, and security buffers
//! - **Event logging**: Write structured events to Android's event log
//! - **Persistent logging**: Optional logging to pstore (survives reboots on Android)
//! - **Runtime configuration**: Adjust log levels, tags, and filters after initialization
//! - **Cross-platform**: Works on Android and falls back to stderr on other platforms
//!
//! # Quick Start
//!
//! ```
//! use log::{debug, error, info, trace, warn};
//! android_logd_logger::builder()
//!     .parse_filters("debug")
//!     .tag("MyApp")
//!     .prepend_module(true)
//!     .init();
//!
//! trace!("trace message: is not logged");
//! debug!("debug message");
//! info!("info message");
//! warn!("warn message");
//! error!("error message");
//! ```
//!
//! # Runtime Configuration
//!
//! The logger can be reconfigured at runtime using the [`Logger`] handle:
//!
//! ```
//! use log::LevelFilter;
//!
//! let logger = android_logd_logger::builder().init();
//!
//! // Change the tag at runtime
//! logger.tag("NewTag");
//!
//! // Adjust log levels
//! logger.filter_level(LevelFilter::Warn);
//! ```
//!
//! # Event Logging
//!
//! Android's event log can be used for structured logging:
//!
//! ```
//! use android_logd_logger::{write_event_now, EventValue};
//!
//! // Simple event
//! write_event_now(1, "test").unwrap();
//!
//! // Complex event with multiple values
//! let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()];
//! write_event_now(2, value).unwrap();
//! ```

#![deny(missing_docs)]

use env_logger::filter::Builder as FilterBuilder;
use log::{set_boxed_logger, LevelFilter, SetLoggerError};
use logger::Configuration;
use parking_lot::RwLock;
use std::{fmt, io, sync::Arc, time::SystemTime};
use thiserror::Error;

mod events;
#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
mod logd;
mod logger;
#[cfg(target_os = "android")]
mod logging_iterator;
#[cfg(target_os = "android")]
mod pmsg;
mod thread;

pub use events::*;

/// Logger configuration handle.
pub use logger::Logger;

/// Maximum log entry length in bytes (5KB).
const LOGGER_ENTRY_MAX_LEN: usize = 5 * 1024;

/// Errors that can occur when logging.
#[derive(Error, Debug)]
pub enum Error {
    /// IO error
    #[error("IO error")]
    Io(#[from] io::Error),
    /// The supplied event data exceed the maximum length
    #[error("Event exceeds maximum size")]
    EventSize,
    /// Timestamp error
    #[error("Timestamp error: {0}")]
    Timestamp(String),
}

/// Log priority levels as defined by Android's logd.
///
/// These priority levels correspond to Android's logging levels and are used
/// to categorize log messages by severity. The standard Rust log levels are
/// automatically mapped to these Android priorities.
///
/// # Mapping from Rust log levels
///
/// - `log::Level::Error` → `Priority::Error`
/// - `log::Level::Warn` → `Priority::Warn`
/// - `log::Level::Info` → `Priority::Info`
/// - `log::Level::Debug` → `Priority::Debug`
/// - `log::Level::Trace` → `Priority::Verbose`
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Priority {
    /// Unknown priority (internal use only, not for application use).
    _Unknown = 0,

    /// Default priority (internal use only, not for application use).
    _Default = 1,

    /// Verbose log level - detailed diagnostic information.
    Verbose = 2,

    /// Debug log level - debugging information useful during development.
    Debug = 3,

    /// Info log level - informational messages about normal operation.
    Info = 4,

    /// Warning log level - warning messages about potential issues.
    Warn = 5,

    /// Error log level - error messages about failures.
    Error = 6,

    /// Fatal log level (internal use only, not for application use).
    _Fatal = 7,

    /// Silent priority (internal use only, not for application use).
    _Silent = 8,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match self {
            Priority::_Unknown => 'U',
            Priority::_Default | Priority::Debug => 'D',
            Priority::Verbose => 'V',
            Priority::Info => 'I',
            Priority::Warn => 'W',
            Priority::Error => 'E',
            Priority::_Fatal => 'F',
            Priority::_Silent => 'S',
        };
        f.write_str(&c.to_string())
    }
}

impl From<log::Level> for Priority {
    fn from(l: log::Level) -> Priority {
        match l {
            log::Level::Error => Priority::Error,
            log::Level::Warn => Priority::Warn,
            log::Level::Info => Priority::Info,
            log::Level::Debug => Priority::Debug,
            log::Level::Trace => Priority::Verbose,
        }
    }
}

/// Android log buffer identifiers.
///
/// Android maintains multiple ring buffers for different types of logs.
/// Most applications should use [`Buffer::Main`], which is the standard
/// application log buffer.
///
/// # Buffer Types
///
/// - **Main**: Standard application logs (default)
/// - **Radio**: Radio/telephony related logs (system use)
/// - **Events**: Binary event logs for system events
/// - **System**: System component logs
/// - **Crash**: Crash logs
/// - **Stats**: Statistics logs
/// - **Security**: Security-related logs
/// - **Custom**: User-defined buffer ID
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Buffer {
    /// The main log buffer. This is the default and only buffer available to regular apps.
    Main,
    /// The radio log buffer for telephony-related logs (typically system use only).
    Radio,
    /// The event log buffer for structured binary events.
    Events,
    /// The system log buffer for system component logs.
    System,
    /// The crash log buffer for crash reports.
    Crash,
    /// The statistics log buffer for system statistics.
    Stats,
    /// The security log buffer for security-related events.
    Security,
    /// A custom buffer with a user-defined ID.
    Custom(u8),
}

impl From<Buffer> for u8 {
    fn from(b: Buffer) -> u8 {
        match b {
            Buffer::Main => 0,
            Buffer::Radio => 1,
            Buffer::Events => 2,
            Buffer::System => 3,
            Buffer::Crash => 4,
            Buffer::Stats => 5,
            Buffer::Security => 6,
            Buffer::Custom(id) => id,
        }
    }
}

/// Internal tag mode configuration.
///
/// Determines how log tags are generated from log records.
#[derive(Debug, Default, Clone)]
enum TagMode {
    /// Use the record's target metadata as the tag (full module path).
    Target,
    /// Use the root module as tag by stripping everything after the first `::`.
    /// For example, `crate::module::submodule` becomes `crate`.
    #[default]
    TargetStrip,
    /// Use a custom fixed tag string for all log messages.
    Custom(String),
}

/// Internal logging record structure.
///
/// This structure is built once per log call and contains all the information
/// needed to write to both `logd` and `pmsg` devices. By building it once,
/// we ensure consistent timestamps and avoid duplicate system calls.
struct Record<'tag, 'msg> {
    /// Timestamp when the log was created.
    timestamp: SystemTime,
    /// Process ID.
    pid: u16,
    /// Thread ID.
    thread_id: u16,
    /// Target log buffer.
    buffer_id: Buffer,
    /// Log tag string.
    tag: &'tag str,
    /// Log priority level.
    priority: Priority,
    /// Log message content.
    message: &'msg str,
}

/// Returns a default [`Builder`] for configuration and initialization of logging.
///
/// With the help of the [`Builder`] the logging is configured.
/// The tag, filter and buffer can be set.
/// Additionally it is possible to set whether the modul path appears in a log message.
///
/// After a call to [`init`](Builder::init) the global logger is initialized with the configuration.
pub fn builder() -> Builder {
    Builder::default()
}

/// Builder for initializing the logger.
///
/// The builder provides a fluent API for configuring the logger before initialization.
/// It allows setting the log tag, filters, buffer, and other options.
///
/// # Examples
///
/// ```
/// use log::LevelFilter;
/// use android_logd_logger::Builder;
///
/// let logger = Builder::new()
///     .tag("MyApp")
///     .filter_level(LevelFilter::Debug)
///     .prepend_module(true)
///     .init();
/// ```
pub struct Builder {
    filter: FilterBuilder,
    tag: TagMode,
    prepend_module: bool,
    pstore: bool,
    buffer: Option<Buffer>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            filter: FilterBuilder::default(),
            tag: TagMode::default(),
            prepend_module: false,
            pstore: true,
            buffer: None,
        }
    }
}

impl Builder {
    /// Initializes the log builder with defaults.
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.filter(None, LevelFilter::Info).init();
    /// ```
    ///
    /// [`filter`]: #method.filter
    pub fn new() -> Builder {
        Builder::default()
    }

    /// Use a specific android log buffer. Defaults to the main buffer
    /// is used as tag (if present).
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    /// # use android_logd_logger::Buffer;
    ///
    /// let mut builder = Builder::new();
    /// builder.buffer(Buffer::Crash)
    ///     .init();
    /// ```
    pub fn buffer(&mut self, buffer: Buffer) -> &mut Self {
        self.buffer = Some(buffer);
        self
    }

    /// Use a specific log tag. If no tag is set the module path
    /// is used as tag (if present).
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.tag("foo")
    ///     .init();
    /// ```
    pub fn tag(&mut self, tag: &str) -> &mut Self {
        self.tag = TagMode::Custom(tag.to_string());
        self
    }

    /// Use the target string as tag
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.tag_target().init();
    /// ```
    pub fn tag_target(&mut self) -> &mut Self {
        self.tag = TagMode::Target;
        self
    }

    /// Use the target string as tag and strip off ::.*
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.tag_target_strip().init();
    /// ```
    pub fn tag_target_strip(&mut self) -> &mut Self {
        self.tag = TagMode::TargetStrip;
        self
    }

    /// Prepend module to log message.
    ///
    /// If set true the Rust module path is prepended to the log message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.prepend_module(true).init();
    /// ```
    pub fn prepend_module(&mut self, prepend_module: bool) -> &mut Self {
        self.prepend_module = prepend_module;
        self
    }

    /// Adds a directive to the filter for a specific module.
    ///
    /// # Examples
    ///
    /// Only include messages for warning and above for logs in `path::to::module`:
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    /// ```
    pub fn filter_module(&mut self, module: &str, level: LevelFilter) -> &mut Self {
        self.filter.filter_module(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
    ///
    /// # Examples
    ///
    /// Only include messages for warning and above.
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.filter_level(LevelFilter::Info).init();
    /// ```
    pub fn filter_level(&mut self, level: LevelFilter) -> &mut Self {
        self.filter.filter_level(level);
        self
    }

    /// Adds filters to the logger.
    ///
    /// The given module (if any) will log at most the specified level provided.
    /// If no module is provided then the filter will apply to all log messages.
    ///
    /// # Examples
    ///
    /// Only include messages for warning and above for logs in `path::to::module`:
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.filter(Some("path::to::module"), LevelFilter::Info).init();
    /// ```
    pub fn filter(&mut self, module: Option<&str>, level: LevelFilter) -> &mut Self {
        self.filter.filter(module, level);
        self
    }

    /// Parses the directives string in the same form as the `RUST_LOG`
    /// environment variable.
    ///
    /// See the module documentation for more details.
    pub fn parse_filters(&mut self, filters: &str) -> &mut Self {
        self.filter.parse(filters);
        self
    }

    /// Enables or disables logging to the pstore filesystem.
    ///
    /// Messages logged to the pstore filesystem survive a reboot but not a
    /// power cycle. By default, logging to the pstore is enabled.
    #[cfg(target_os = "android")]
    pub fn pstore(&mut self, log_to_pstore: bool) -> &mut Self {
        self.pstore = log_to_pstore;
        self
    }

    /// Initializes the global logger with the built logd logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Errors
    ///
    /// This function will fail if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn try_init(&mut self) -> Result<Logger, SetLoggerError> {
        let configuration = Configuration {
            filter: self.filter.build(),
            tag: self.tag.clone(),
            prepend_module: self.prepend_module,
            pstore: self.pstore,
            buffer_id: self.buffer.unwrap_or(Buffer::Main),
        };
        let max_level = configuration.filter.filter();
        let configuration = Arc::new(RwLock::new(configuration));

        let logger = Logger {
            configuration: configuration.clone(),
        };
        let logger_impl = logger::LoggerImpl::new(configuration).expect("failed to build logger");

        set_boxed_logger(Box::new(logger_impl))
            .map(|_| {
                log::set_max_level(max_level);
            })
            .map(|_| logger)
    }

    /// Initializes the global logger with the built logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Panics
    ///
    /// This function will panic if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn init(&mut self) -> Logger {
        self.try_init()
            .expect("Builder::init should not be called after logger initialized")
    }
}

/// Construct and send a log entry directly to the logd socket.
///
/// This function allows you to create custom log entries with explicit control
/// over all parameters including timestamp, buffer, priority, process/thread IDs,
/// tag, and message. This is useful for forwarding logs from other sources or
/// creating synthetic log entries.
///
/// On Android, this writes directly to the logd socket. On other platforms,
/// it prints to stderr in logcat format.
///
/// # Parameters
///
/// - `timestamp`: The timestamp for the log entry
/// - `buffer_id`: The target log buffer
/// - `priority`: The log priority level
/// - `pid`: Process ID to associate with the log
/// - `thread_id`: Thread ID to associate with the log
/// - `tag`: The log tag string
/// - `message`: The log message content
///
/// # Errors
///
/// Returns an error if the log entry cannot be written.
///
/// # Example
///
/// ```
/// # use android_logd_logger::{Buffer, Priority};
/// # use std::time::SystemTime;
///
/// android_logd_logger::log(SystemTime::now(), Buffer::Main, Priority::Info, 0, 0, "tag", "message").unwrap();
/// ```
#[cfg(target_os = "android")]
pub fn log(
    timestamp: SystemTime,
    buffer_id: Buffer,
    priority: Priority,
    pid: u16,
    thread_id: u16,
    tag: &str,
    message: &str,
) -> Result<(), Error> {
    let record = Record {
        timestamp,
        pid,
        thread_id,
        buffer_id,
        tag,
        priority,
        message,
    };

    logd::log(&record);

    Ok(())
}

/// Construct and send a log entry (non-Android platforms).
///
/// This function allows you to create custom log entries with explicit control
/// over all parameters. On non-Android platforms, it prints to stderr in logcat format.
///
/// # Parameters
///
/// - `timestamp`: The timestamp for the log entry
/// - `buffer_id`: The target log buffer
/// - `priority`: The log priority level
/// - `pid`: Process ID to associate with the log
/// - `thread_id`: Thread ID to associate with the log
/// - `tag`: The log tag string
/// - `message`: The log message content
///
/// # Errors
///
/// Returns an error if the log entry cannot be formatted or written.
///
/// # Example
///
/// ```
/// # use android_logd_logger::{Buffer, Priority};
/// # use std::time::SystemTime;
///
/// android_logd_logger::log(SystemTime::now(), Buffer::Main, Priority::Info, 0, 0, "tag", "message").unwrap();
/// ```
#[cfg(not(target_os = "android"))]
pub fn log(
    timestamp: SystemTime,
    buffer_id: Buffer,
    priority: Priority,
    pid: u16,
    thread_id: u16,
    tag: &str,
    message: &str,
) -> Result<(), Error> {
    let record = Record {
        timestamp,
        pid,
        thread_id,
        buffer_id,
        tag,
        priority,
        message,
    };

    log_record(&record)
}

#[cfg(target_os = "android")]
fn log_record(record: &Record) -> Result<(), Error> {
    logd::log(record);
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn log_record(record: &Record) -> Result<(), Error> {
    use std::time::UNIX_EPOCH;

    const DATE_TIME_FORMAT: &[time::format_description::FormatItem<'_>] =
        time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]");

    let Record {
        timestamp,
        tag,
        priority,
        message,
        thread_id,
        pid,
        ..
    } = record;

    let timestamp = timestamp
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Timestamp(e.to_string()))
        .and_then(|ts| {
            time::OffsetDateTime::from_unix_timestamp_nanos(ts.as_nanos() as i128).map_err(|e| Error::Timestamp(e.to_string()))
        })
        .and_then(|ts| ts.format(&DATE_TIME_FORMAT).map_err(|e| Error::Timestamp(e.to_string())))?;

    eprintln!("{} {} {} {} {}: {}", timestamp, pid, thread_id, priority, tag, message);
    Ok(())
}

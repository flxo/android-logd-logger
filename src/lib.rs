//! `android-logd-logger`

#![deny(missing_docs)]

use env_logger::filter::Builder as FilterBuilder;
use log::{set_boxed_logger, LevelFilter, SetLoggerError};
use logger::Configuration;
use parking_lot::RwLock;
use std::{fmt, io, process, sync::Arc, time::SystemTime};
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

/// Max log entry len.
const LOGGER_ENTRY_MAX_LEN: usize = 5 * 1024;

/// Error
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

/// Log priority as defined by logd
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Priority {
    /// For internal logd use only
    _Unknown = 0,

    /// For internal logd use only
    _Default = 1,

    /// Android verbose log level
    Verbose = 2,

    /// Android debug log level
    Debug = 3,

    /// Android info log level
    Info = 4,

    /// Android warning log level
    Warn = 5,

    /// Android error log level
    Error = 6,

    /// Android fatal log level
    _Fatal = 7,

    /// For internal logd use only
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

/// Log buffer ids
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Buffer {
    /// The main log buffer. This is the only log buffer available to apps.
    Main,
    /// The radio log buffer
    Radio,
    /// The event log buffer.
    Events,
    /// The system log buffer.
    System,
    /// The crash log buffer.
    Crash,
    /// The statistics log buffer.
    Stats,
    /// The security log buffer.
    Security,
    /// User defined Buffer
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

/// Tag mode
#[derive(Debug, Default, Clone)]
enum TagMode {
    /// Use the records target metadata as tag
    Target,
    /// Use root module as tag. The target field contains the module path
    /// if not overwritten. Use the root module as tag. e.g a target of
    /// `crate::module::submodule` will be `crate`.
    #[default]
    TargetStrip,
    /// Custom fixed tag string
    Custom(String),
}

/// Logging record structure
///
/// We build this structure in the [`Logger`] per `log()` call and pass
/// consistent timestamps and other information to both the `logd` and the
/// `pmsg` device without paying the price for system calls twice.
struct Record<'tag, 'msg> {
    timestamp: SystemTime,
    buffer_id: Buffer,
    tag: &'tag str,
    priority: Priority,
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

/// Builder for initializing logger
///
/// The builder is used to initialize the logging framework for later use.
/// It provides
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

/// Construct a log entry and send it to the logd writer socket
///
/// This can be used to forge an android logd entry
///
/// # Example
///
/// ```
/// # use android_logd_logger::{Buffer, Priority};
/// # use std::time::SystemTime;
///
/// android_logd_logger::log(SystemTime::now(), Buffer::Main, Priority::Info, "tag", "message").unwrap();
/// ```
#[cfg(target_os = "android")]
pub fn log(timestamp: SystemTime, buffer_id: Buffer, priority: Priority, tag: &str, message: &str) -> Result<(), Error> {
    let record = Record {
        timestamp,
        buffer_id,
        tag,
        priority,
        message,
    };

    logd::log(&record);

    Ok(())
}

/// Construct a log entry
///
/// This can be used to forge an android logd entry
///
/// # Example
///
/// ```
/// # use android_logd_logger::{Buffer, Priority};
/// # use std::time::SystemTime;
///
/// android_logd_logger::log(SystemTime::now(), Buffer::Main, Priority::Info, "tag", "message").unwrap();
/// ```
#[cfg(not(target_os = "android"))]
pub fn log(timestamp: SystemTime, buffer_id: Buffer, priority: Priority, tag: &str, message: &str) -> Result<(), Error> {
    let record = Record {
        timestamp,
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
        ..
    } = record;

    let timestamp = timestamp
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Timestamp(e.to_string()))
        .and_then(|ts| {
            time::OffsetDateTime::from_unix_timestamp_nanos(ts.as_nanos() as i128).map_err(|e| Error::Timestamp(e.to_string()))
        })
        .and_then(|ts| ts.format(&DATE_TIME_FORMAT).map_err(|e| Error::Timestamp(e.to_string())))?;

    eprintln!(
        "{} {} {} {} {}: {}",
        timestamp,
        process::id(),
        thread::id(),
        priority,
        tag,
        message
    );
    Ok(())
}

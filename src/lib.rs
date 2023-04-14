//! # `android-logd-logger`
//!
//! [![Crates.io][crates-badge]][crates-url]
//! [![Build Status][actions-badge]][actions-url]
//! [![Docs][docs-badge]][docs-url]
//!
//! [docs-badge]: https://docs.rs/android-logd-logger/badge.svg
//! [docs-url]: https://docs.rs/android-logd-logger
//! [crates-badge]: https://img.shields.io/crates/v/android-logd-logger.svg
//! [crates-url]: https://crates.io/crates/android-logd-logger
//! [actions-badge]: https://github.com/flxo/android-logd-logger/workflows/CI/badge.svg
//! [actions-url]: https://github.com/flxo/android-logd-logger/actions?query=workflow%3ACI+branch%3Amaster
//!
//! This logger writes logs to the Android `logd`, a system service with
//! multiple ringbuffers for logs and evens. This is normally done
//! via `liblog` (a native Android lib). Instead of using `liblog`, this crate
//! writes directly to the `logd` socket with the trivial protocol below.
//! This logger is written in pure Rust without any need for ffi.
//!
//! [log]: https://docs.rs/log/*/log/
//! [`error!`]: https://docs.rs/log/*/log/macro.error.html
//! [`warn!`]: https://docs.rs/log/*/log/macro.warn.html
//! [`info!`]: https://docs.rs/log/*/log/macro.info.html
//! [`debug!`]: https://docs.rs/log/*/log/macro.debug.html
//! [`trace!`]: https://docs.rs/log/*/log/macro.trace.html
//!
//! On non Android system the log output is printed to stdout in the default
//! format of `logcat`.
//!
//! # Usage
//!
//! Add this to your Cargo.toml
//!
//! ```toml
//! [dependencies]
//! android-logd-logger = "0.2.1"
//! ```
//!
//! Initialize the logger with a fixed `tag` and the module path included
//! in the log payload.
//!
//! ```
//! # use log::*;
//!
//! fn main() {
//!     android_logd_logger::builder()
//!         .parse_filters("debug")
//!         .tag("log_tag")
//!         .prepend_module(true)
//!         .init();
//!
//!     trace!("trace message: is not logged");
//!     debug!("debug message");
//!     info!("info message");
//!     warn!("warn message");
//!     error!("error message");
//! }
//! ```
//!
//! To write android logd "events" use `event` or `event_now`, e.g:
//!
//! ```
//! android_logd_logger::write_event_now(1, "test").unwrap();
//! ```
//!
//! # Configuration
//!
//! Writing to the logd socket is a single point of synchronization for threads.
//! The `android-logd-logger` can be configured with the `tls` feature to maintain
//! one socket per thread *or* use a single socket for the whole process.
//! Use the features `tls` if you want less interference between threads but pay
//! for one connection per thread.
//!
//! # License
//!
//! Licensed under either of
//!
//!  * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
//!  * MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

#![deny(missing_docs)]

use env_logger::filter::Builder as FilterBuilder;
use log::{set_boxed_logger, LevelFilter, SetLoggerError};
use logger::Configuration;
use parking_lot::RwLock;
use std::{fmt, io, sync::Arc};
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
}

/// Log priority as defined by logd
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Priority {
    _Unknown = 0,
    _Default = 1,
    Verbose = 2,
    Debug = 3,
    Info = 4,
    Warn = 5,
    Error = 6,
    _Fatal = 7,
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
    timestamp_secs: u32,
    timestamp_subsec_nanos: u32,
    #[allow(unused)]
    pid: u16,
    #[allow(unused)]
    thread_id: u16,
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

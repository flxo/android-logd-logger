//! # `android-logd-logger`
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
//! android-logd-logger = "0.1.2"
//! ```
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
//!
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
//!  * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
//!  * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

#![deny(missing_docs)]
use env_logger::filter::{Builder as FilterBuilder, Filter};
#[cfg(all(not(feature = "tls"), target_os = "android"))]
use lazy_static::lazy_static;
use log::{LevelFilter, Log, Metadata, SetLoggerError};
use std::{fmt, io};
#[cfg(target_os = "android")]
use {bytes::BufMut, std::os::unix::net::UnixDatagram};

#[cfg(target_os = "android")]
const LOGDW: &str = "/dev/socket/logdw";

#[cfg(all(feature = "tls", target_os = "android"))]
thread_local! {
     static SOCKET: UnixDatagram = {
        let socket = UnixDatagram::unbound().expect("Failed to create socket");
        socket.connect(LOGDW).expect("Failed to connect to /dev/socket/logdw");
        socket
    };
}
#[cfg(all(not(feature = "tls"), target_os = "android"))]
lazy_static! {
    static ref SOCKET: UnixDatagram = {
        let socket = UnixDatagram::unbound().expect("Failed to create socket");
        socket.connect(LOGDW).expect("Failed to connect to /dev/socket/logdw");
        socket
    };
}

mod thread {
    #[cfg(unix)]
    #[inline]
    pub fn id() -> usize {
        unsafe { libc::pthread_self() as usize }
    }

    #[cfg(windows)]
    #[inline]
    pub fn id() -> usize {
        unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as usize }
    }

    #[cfg(target_os = "redox")]
    #[inline]
    pub fn id() -> usize {
        // Each thread has a separate pid on Redox.
        syscall::getpid().unwrap()
    }
}

//#[cfg(target_os = "android")]
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
    /// Main Buffer with id 0
    Main,
    /// Radio Buffer with id 1
    Radio,
    /// Event Buffer with id 2
    Events,
    /// System Buffer with id 3
    System,
    /// Crash Buffer with id 4
    Crash,
    /// Kernel Buffer with id 5
    Kernel,
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
            Buffer::Kernel => 5,
            Buffer::Custom(id) => id,
        }
    }
}

/// Builder for initializing logger
///
/// The builder is used to initialize the logging framework for later use.
/// It provides
#[derive(Default)]
pub struct Builder<'a> {
    filter: FilterBuilder,
    tag: Option<&'a str>,
    prepend_module: Option<bool>,
    buffer: Option<Buffer>,
}

impl<'a> Builder<'a> {
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
    ///
    /// builder.filter(None, LevelFilter::Info)
    ///        .init();
    /// ```
    ///
    /// [`filter`]: #method.filter
    pub fn new() -> Builder<'a> {
        Builder::default()
    }

    /// Use a specific android log buffer. Defaults to the main buffer
    /// is used as tag (if present)
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    /// # use android_logd_logger::Buffer;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash)
    ///     .init();
    /// ```
    pub fn buffer(&mut self, buffer: Buffer) -> &mut Self {
        self.buffer = Some(buffer);
        self
    }

    /// Use a specific log tag. If no tag is set the module path
    /// is used as tag (if present)
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
    pub fn tag(&mut self, tag: &'a str) -> &mut Self {
        self.tag = Some(tag);
        self
    }

    /// Prepend module to log message. If set true the Rust module path
    /// is prepended to the log message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use android_logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.prepend_module(true)
    ///     .init();
    /// ```
    pub fn prepend_module(&mut self, prepend: bool) -> &mut Self {
        self.prepend_module = Some(prepend);
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
    /// builder.filter_module("path::to::module", LevelFilter::Info);
    /// ```
    pub fn filter_module(&mut self, module: &str, level: LevelFilter) -> &mut Self {
        self.filter.filter_module(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
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
    /// builder.filter_level(LevelFilter::Info);
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
    ///
    /// builder.filter(Some("path::to::module"), LevelFilter::Info);
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

    /// Initializes the global logger with the built logd logger.
    ///
    /// This should be called early in the execution of a Rust program. Any log
    /// events that occur before initialization will be ignored.
    ///
    /// # Errors
    ///
    /// This function will fail if it is called more than once, or if another
    /// library has already initialized a global logger.
    pub fn try_init(&mut self) -> Result<(), SetLoggerError> {
        let logger = self.build();

        let max_level = logger.filter.filter();
        let r = log::set_boxed_logger(Box::new(logger));

        if r.is_ok() {
            log::set_max_level(max_level);
        }

        r
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
    pub fn init(&mut self) {
        self.try_init()
            .expect("Builder::init should not be called after logger initialized");
    }

    fn build(&mut self) -> Logger {
        let buffer = self.buffer.unwrap_or(Buffer::Main);
        let filter = self.filter.build();
        let prepend_module = self.prepend_module.unwrap_or(false);
        Logger::new(buffer, filter, self.tag.map(str::to_string), prepend_module).expect("Failed to build logger")
    }
}

struct Logger {
    filter: Filter,
    tag: Option<String>,
    prepend_module: bool,
    _buffer_id: Buffer,
}

impl Logger {
    pub fn new(buffer_id: Buffer, filter: Filter, tag: Option<String>, prepend_module: bool) -> Result<Logger, io::Error> {
        Ok(Logger {
            filter,
            tag,
            prepend_module,
            _buffer_id: buffer_id,
        })
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.filter.matches(record) {
            return;
        }

        let args = record.args().to_string();
        let message = if let Some(module_path) = record.module_path() {
            if self.prepend_module {
                let mut message = String::with_capacity(module_path.len() + args.len());
                message.push_str(module_path);
                message.push_str(": ");
                message.push_str(&args);
                message
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();

        #[cfg(target_os = "android")]
        {
            let timestamp = std::time::SystemTime::now();

            let tag = if let Some(ref tag) = self.tag {
                tag
            } else {
                record.module_path().unwrap_or_default()
            };

            let tag_len = tag.bytes().len() + 1;
            let message_len = message.bytes().len() + 1;

            let mut buffer = bytes::BytesMut::with_capacity(12 + tag_len + message_len);

            buffer.put_u8(self._buffer_id.into());
            buffer.put_u16_le(thread::id() as u16);
            let timestamp = timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Failed to aquire time");
            buffer.put_u32_le(timestamp.as_secs() as u32);
            buffer.put_u32_le(timestamp.subsec_nanos());
            buffer.put_u8(priority as u8);
            buffer.put(tag.as_bytes());
            buffer.put_u8(0);

            buffer.put(message.as_bytes());
            buffer.put_u8(0);

            #[cfg(feature = "tls")]
            SOCKET.with(|f| f.send(&buffer).expect("Logd socket error"));

            #[cfg(not(feature = "tls"))]
            SOCKET.send(&buffer).expect("Logd socket error");
        }

        #[cfg(not(target_os = "android"))]
        {
            let datetime = ::time::OffsetDateTime::now_utc();
            let tag = if let Some(ref tag) = self.tag {
                tag
            } else {
                record.module_path().unwrap_or_default()
            };
            println!(
                "{}.{} {} {} {} {}: {}",
                datetime.format("%Y-%m-%d %T"),
                &datetime.format("%N")[..3],
                std::process::id(),
                thread::id(),
                priority,
                tag,
                message
            );
        }
    }

    #[cfg(not(target_os = "android"))]
    fn flush(&self) {
        use std::io::Write;
        io::stdout().flush().ok();
    }

    #[cfg(target_os = "android")]
    fn flush(&self) {}
}

/// Returns a default [`Builder`] for configuration and initialization of logging.
///
/// With the help of the [`Builder`] the logging is configured.
/// The tag, filter and buffer can be set.
/// Additionally it is possible to set whether the modul path appears in a log message.
///
/// After a call to [`init`](Builder::init) the global logger is initialized with the configuration.
pub fn builder<'a>() -> Builder<'a> {
    Builder::default()
}

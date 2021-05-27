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
//!  * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
//!  * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

#![deny(missing_docs)]

use bytes::{BufMut, Bytes, BytesMut};
use env_logger::filter::{Builder as FilterBuilder, Filter};
#[cfg(all(not(feature = "tls"), target_os = "android"))]
use lazy_static::lazy_static;
use log::{LevelFilter, Log, Metadata, SetLoggerError};
#[cfg(target_os = "android")]
use std::os::unix::net::UnixDatagram;
use std::{fmt, io, iter::FromIterator, time::SystemTime};
use thiserror::Error;

#[cfg(target_os = "android")]
const LOGDW: &str = "/dev/socket/logdw";

/// Max log entry len
const LOGGER_ENTRY_MAX_LEN: usize = 5 * 1024;

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
            let timestamp = SystemTime::now();

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

/// Event tag
pub type EventTag = u32;

/// Event data
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Timestamp
    pub timestamp: SystemTime,
    /// Tag
    pub tag: EventTag,
    /// Value
    pub value: EventValue,
}

/// Event's value
#[derive(Debug, PartialEq, Clone)]
pub enum EventValue {
    /// Int value
    Int(i32),
    /// Long value
    Long(i64),
    /// Float value
    Float(f32),
    /// String value
    String(String),
    /// List of values
    List(Vec<EventValue>),
}

impl EventValue {
    /// Serialied size
    pub fn serialized_size(&self) -> usize {
        match self {
            EventValue::Int(_) | EventValue::Float(_) => 1 + 4,
            EventValue::Long(_) => 1 + 8,
            EventValue::String(s) => 1 + 4 + s.as_bytes().len(),
            EventValue::List(l) => 1 + 1 + l.iter().map(EventValue::serialized_size).sum::<usize>(),
        }
    }

    /// Serialize the event value into bytes
    pub fn as_bytes(&self) -> Bytes {
        const EVENT_TYPE_INT: u8 = 0;
        const EVENT_TYPE_LONG: u8 = 1;
        const EVENT_TYPE_STRING: u8 = 2;
        const EVENT_TYPE_LIST: u8 = 3;
        const EVENT_TYPE_FLOAT: u8 = 4;

        let mut buffer = BytesMut::with_capacity(self.serialized_size());
        match self {
            EventValue::Int(num) => {
                buffer.put_u8(EVENT_TYPE_INT);
                buffer.put_i32_le(*num);
            }
            EventValue::Long(num) => {
                buffer.put_u8(EVENT_TYPE_LONG);
                buffer.put_i64_le(*num);
            }
            EventValue::Float(num) => {
                buffer.put_u8(EVENT_TYPE_FLOAT);
                buffer.put_f32_le(*num);
            }
            EventValue::String(string) => {
                buffer.put_u8(EVENT_TYPE_STRING);
                buffer.put_u32_le(string.len() as u32);
                buffer.put(string.as_bytes());
            }
            EventValue::List(values) => {
                buffer.put_u8(EVENT_TYPE_LIST);
                buffer.put_u8(values.len() as u8);
                values.iter().for_each(|value| buffer.put(value.as_bytes()));
            }
        };
        buffer.freeze()
    }
}

impl From<i32> for EventValue {
    fn from(v: i32) -> Self {
        EventValue::Int(v)
    }
}

impl From<i64> for EventValue {
    fn from(v: i64) -> Self {
        EventValue::Long(v)
    }
}

impl From<f32> for EventValue {
    fn from(v: f32) -> Self {
        EventValue::Float(v)
    }
}

impl From<&str> for EventValue {
    fn from(v: &str) -> Self {
        EventValue::String(v.to_string())
    }
}

impl<T> FromIterator<T> for EventValue
where
    T: Into<EventValue>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        EventValue::List(iter.into_iter().map(Into::into).collect())
    }
}

impl<T> From<Vec<T>> for EventValue
where
    T: Into<EventValue>,
{
    fn from(mut v: Vec<T>) -> Self {
        EventValue::List(v.drain(..).map(|e| e.into()).collect())
    }
}

/// Write an event with the timestamp now to `Buffer::Events`
/// ```
/// use android_logd_logger::{write_event, write_event_now, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_now(1, "test").unwrap();
///
/// let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()].into();
/// write_event_now(2, value).unwrap();
/// ```
pub fn write_event_now<T: Into<EventValue>>(tag: EventTag, value: T) -> Result<(), Error> {
    write_event(&Event {
        timestamp: SystemTime::now(),
        tag,
        value: value.into(),
    })
}

/// Write an event with the timestamp now to buffer
/// ```
/// use android_logd_logger::{write_event_buffer_now, Buffer, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_buffer_now(Buffer::Stats, 1, "test").unwrap();
///
/// let value: Vec<EventValue> = vec![1.into(), "one".into(), 123.3.into()].into();
/// write_event_buffer_now(Buffer::Stats, 2, value).unwrap();
/// ```
pub fn write_event_buffer_now<T: Into<EventValue>>(log_buffer: Buffer, tag: EventTag, value: T) -> Result<(), Error> {
    write_event_buffer(
        log_buffer,
        &Event {
            timestamp: SystemTime::now(),
            tag,
            value: value.into(),
        },
    )
}

/// Write an event to `Buffer::Events`
/// ```
/// use android_logd_logger::{write_event, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event(&Event {
///     timestamp: std::time::SystemTime::now(),
///     tag: 1,
///     value: "blah".into(),
/// }).unwrap();
/// ```
pub fn write_event(event: &Event) -> Result<(), Error> {
    write_event_buffer(Buffer::Events, event)
}

/// Write an event to an explicit buffer
/// ```
/// use android_logd_logger::{write_event_buffer, Buffer, Error, Event, EventValue};
/// android_logd_logger::builder().init();
///
/// write_event_buffer(Buffer::Stats, &Event {
///     timestamp: std::time::SystemTime::now(),
///     tag: 1,
///     value: "blah".into(),
/// }).unwrap();
/// ```
pub fn write_event_buffer(log_buffer: Buffer, event: &Event) -> Result<(), Error> {
    if event.value.serialized_size() > (LOGGER_ENTRY_MAX_LEN - 1 - 2 - 4 - 4 - 4) {
        return Err(Error::EventSize);
    }

    #[cfg(target_os = "android")]
    {
        let mut buffer = bytes::BytesMut::with_capacity(LOGGER_ENTRY_MAX_LEN);
        let timestamp = event.timestamp.elapsed().unwrap();

        buffer.put_u8(log_buffer.into());
        buffer.put_u16_le(thread::id() as u16);
        buffer.put_u32_le(timestamp.as_secs() as u32);
        buffer.put_u32_le(timestamp.subsec_nanos());
        buffer.put_u32_le(event.tag);
        buffer.put(event.value.as_bytes());

        #[cfg(feature = "tls")]
        SOCKET.with(|f| f.send(&buffer).map_err(Error::Io))?;

        #[cfg(not(feature = "tls"))]
        SOCKET.send(&buffer).map_err(Error::Io)?;
    }

    #[cfg(not(target_os = "android"))]
    println!("buffer: {:?}, event: {:?}", log_buffer, event);

    Ok(())
}

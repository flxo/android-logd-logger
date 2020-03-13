use crossbeam::channel::Sender;
use env_logger::filter::{Builder as FilterBuilder, Filter};
use log::{LevelFilter, Log, Metadata, SetLoggerError};
use std::fmt;
use std::time;

mod thread;

#[cfg(target_os = "android")]
const LOGD_WR_SOCKET: &str = "/dev/socket/logdw";

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

/// Log record
#[derive(Debug)]
struct Record {
    /// Local timestamp of record
    timestamp: time::SystemTime,
    /// Log level
    priority: Priority,
    /// Log tag
    tag: Option<String>,
    /// Log message
    message: String,
    /// Thread
    process: u32,
    /// Thread
    thread: u32,
}

/// Log buffer ids
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Buffer {
    Main,
    Radio,
    Events,
    System,
    Crash,
    Kernel,
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

#[derive(Default)]
pub struct Builder<'a> {
    filter: FilterBuilder,
    tag: Option<&'a str>,
    append_module: Option<bool>,
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
    /// use log::LevelFilter;
    /// use logd_logger::Builder;
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
    /// use logd_logger::Builder;
    /// use logd_logger::Buffer;
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
    /// use logd_logger::Builder;
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

    /// Append module to log message. If set true the Rust module path
    /// is appended to the log message.
    ///
    /// # Examples
    ///
    /// ```
    /// use logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.append_module(true)
    ///     .init();
    /// ```
    pub fn append_module(&mut self, append: bool) -> &mut Self {
        self.append_module = Some(append);
        self
    }

    /// Prepend module to log message. If set true the Rust module path
    /// is prepended to the log message.
    ///
    /// # Examples
    ///
    /// ```
    /// use logd_logger::Builder;
    ///
    /// let mut builder = Builder::new();
    ///
    /// builder.prepend_module(true)
    ///     .init();
    /// ```
    pub fn prepend_module(&mut self, append: bool) -> &mut Self {
        self.prepend_module = Some(append);
        self
    }

    /// Adds a directive to the filter for a specific module.
    ///
    /// # Examples
    ///
    /// Only include messages for warning and above for logs in `path::to::module`:
    ///
    /// ```
    /// use log::LevelFilter;
    /// use logd_logger::Builder;
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
    /// use log::LevelFilter;
    /// use logd_logger::Builder;
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
    /// use log::LevelFilter;
    /// use logd_logger::Builder;
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
        let append_module = self.append_module.unwrap_or(false);
        let prepend_module = self.prepend_module.unwrap_or(false);
        Logger::new(buffer, filter, self.tag.map(str::to_string), prepend_module, append_module).expect("Failed to build logger")
    }
}

struct Logger {
    filter: Filter,
    tag: Option<String>,
    prepend_module: bool,
    append_module: bool,
    tx: Sender<Record>,
}

impl Logger {
    pub fn new(
        buffer_id: Buffer,
        filter: Filter,
        tag: Option<String>,
        prepend_module: bool,
        append_module: bool,
    ) -> Result<Logger, std::io::Error> {
        let (tx, rx) = crossbeam::channel::bounded(100);

        #[cfg(target_os = "android")]
        {
            use bytes::BufMut;

            let socket = std::os::unix::net::UnixDatagram::unbound()?;
            socket.connect(LOGD_WR_SOCKET)?;
            let tag = tag.clone();
            std::thread::spawn(move || {
                let mut buffer = bytes::BytesMut::with_capacity(1024);
                loop {
                    let record: Record = rx.recv().expect("Logd channel error");
                    let timestamp = record
                        .timestamp
                        .duration_since(time::UNIX_EPOCH)
                        .expect("Logd timestamp error");

                    let tag_len = if let Some(ref tag) = tag {
                        tag.bytes().len()
                    } else {
                        record.tag.as_ref().map(|s| s.bytes().len()).unwrap_or(0)
                    } + 1;
                    let message_len = record.message.bytes().len() + 1;
                    buffer.reserve(12 + tag_len + message_len);

                    buffer.put_u8(buffer_id.into());
                    buffer.put_u16_le(record.thread as u16);
                    buffer.put_u32_le(timestamp.as_secs() as u32);
                    buffer.put_u32_le(timestamp.subsec_nanos());
                    buffer.put_u8(record.priority as u8);

                    if let Some(ref tag) = tag {
                        buffer.put(tag.as_bytes());
                    } else {
                        buffer.put(record.tag.unwrap_or_default().as_bytes());
                    };
                    buffer.put_u8(0);

                    buffer.put(record.message.as_bytes());
                    buffer.put_u8(0);

                    socket.send(&buffer).expect("Logd socket error");
                    buffer.clear();
                }
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            use chrono::offset::Utc;
            use chrono::DateTime;

            let _ = buffer_id;
            let tag = tag.clone();
            std::thread::spawn(move || loop {
                let record: Record = rx.recv().expect("Logd channel error");
                let timestamp = record.timestamp;
                let tag = if let Some(ref tag) = tag {
                    tag.clone()
                } else {
                    record.tag.unwrap_or_default()
                };
                let message = record.message;
                let datetime: DateTime<Utc> = timestamp.into();
                println!(
                    "{} {} {} {} {}: {}",
                    datetime.format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.process,
                    record.thread,
                    record.priority,
                    tag,
                    message
                )
            });
        }

        Ok(Logger {
            filter,
            tag,
            prepend_module,
            append_module,
            tx,
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
            let mut message = String::new();
            message.reserve(256);

            if self.prepend_module {
                message.push_str(module_path);
                message.push_str(": ");
            }

            message.push_str(&args);

            if self.append_module {
                message.push_str(" (");
                message.push_str(module_path);
                message.push(')');
            }
            message
        } else {
            args
        };

        let process = std::process::id();
        let thread = thread::id() as u32;

        let tag = if let Some(ref tag) = self.tag {
            Some(tag.to_string())
        } else {
            record.module_path().map(str::to_string)
        };

        let record = Record {
            timestamp: std::time::SystemTime::now(),
            priority: record.metadata().level().into(),
            tag,
            message,
            process,
            thread,
        };

        self.tx.send(record).expect("Failed to log");
    }

    fn flush(&self) {}
}

pub fn builder<'a>() -> Builder<'a> {
    Builder::default()
}

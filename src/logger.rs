#[cfg(target_os = "android")]
use crate::{thread, Record};
use crate::{Buffer, Priority, TagMode};
use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
use parking_lot::RwLock;
#[cfg(target_os = "android")]
use std::time::SystemTime;
use std::{io, sync::Arc};

/// Logger configuration.
pub(crate) struct Configuration {
    pub(crate) filter: Filter,
    pub(crate) tag: TagMode,
    pub(crate) prepend_module: bool,
    #[allow(unused)]
    pub(crate) pstore: bool,
    pub(crate) buffer_id: Option<Buffer>,
}

/// Logger configuration handler stores access to logger configuration parameters.
#[derive(Clone)]
pub struct Logger {
    pub(crate) configuration: Arc<RwLock<Configuration>>,
}

impl Logger {
    /// Sets buffer parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::{Builder, Buffer};
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.buffer(Buffer::Crash);
    /// ```
    pub fn buffer(&self, buffer: Buffer) -> &Self {
        self.configuration.write().buffer_id = Some(buffer);
        self
    }

    // Sets tag parameter of logger configuration to custom value
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.tag("foo");
    /// ```
    pub fn tag(&self, tag: &str) -> &Self {
        self.configuration.write().tag = TagMode::Custom(tag.into());
        self
    }

    /// Sets tag parameter of logger configuration to target value
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.tag_target();
    /// ```
    pub fn tag_target(&self) -> &Self {
        self.configuration.write().tag = TagMode::Target;
        self
    }

    /// Sets tag parameter of logger configuration to strip value
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.tag_target_strip();
    /// ```
    pub fn tag_target_strip(&self) -> &Self {
        self.configuration.write().tag = TagMode::TargetStrip;
        self
    }

    /// Sets prepend module parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.prepend_module(true);
    /// ```
    pub fn prepend_module(&self, prepend_module: bool) -> &Self {
        self.configuration.write().prepend_module = prepend_module;
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
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.filter_module("path::to::module", LevelFilter::Info);
    /// ```
    pub fn filter_module(&mut self, module: &str, level: LevelFilter) -> &mut Self {
        self.configuration.write().filter = env_logger::filter::Builder::default().filter_module(module, level).build();
        self
    }

    /// Adjust filter.
    ///
    /// # Examples
    ///
    /// Only include messages for warning and above for logs in `path::to::module`:
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = Builder::new().init();
    /// logger.filter_level(LevelFilter::Info).init();
    /// ```
    pub fn filter_level(&mut self, level: LevelFilter) -> &mut Self {
        self.configuration.write().filter = env_logger::filter::Builder::default().filter_level(level).build();
        self
    }

    /// Adjust filter.
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
    /// let logger = Builder::new().init();
    /// logger.filter(Some("path::to::module"), LevelFilter::Info).init();
    /// ```
    pub fn filter(&mut self, module: Option<&str>, level: LevelFilter) -> &mut Self {
        self.configuration.write().filter = env_logger::filter::Builder::default().filter(module, level).build();
        self
    }

    /// Parses the directives string in the same form as the `RUST_LOG`
    /// environment variable.
    ///
    /// See the module documentation for more details.
    pub fn parse_filters(&mut self, filters: &str) -> &mut Self {
        self.configuration.write().filter = env_logger::filter::Builder::default().parse(filters).build();
        self
    }

    /// Sets filter parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder().init();
    ///
    /// logger.pstore(true);
    /// ```
    #[cfg(target_os = "android")]
    pub fn pstore(&self, pstore: bool) -> &Self {
        self.logger.write().pstore = pstore;
        self
    }
}
pub(crate) struct LoggerImpl {
    configuration: Arc<RwLock<Configuration>>,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl LoggerImpl {
    pub fn new(configuration: Arc<RwLock<Configuration>>) -> Result<LoggerImpl, io::Error> {
        Ok(LoggerImpl {
            configuration,
            #[cfg(not(target_os = "android"))]
            timestamp_format: time::format_description::parse(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
            )
            .unwrap(),
        })
    }
}

impl Log for LoggerImpl {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.configuration.read().filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        let configuration = self.configuration.read();
        if !configuration.filter.matches(record) {
            return;
        }

        let args = record.args().to_string();
        let message = if let Some(module_path) = record.module_path() {
            if configuration.prepend_module {
                [module_path, &args].join(": ")
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();
        let tag = match &configuration.tag {
            TagMode::Target => record.target(),
            TagMode::TargetStrip => record
                .target()
                .split_once("::")
                .map(|(tag, _)| tag)
                .unwrap_or_else(|| record.target()),
            TagMode::Custom(tag) => tag.as_str(),
        };

        #[cfg(target_os = "android")]
        {
            let timestamp = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("failed to acquire time");
            let log_record = Record {
                timestamp_secs: timestamp.as_secs() as u32,
                timestamp_subsec_nanos: timestamp.subsec_nanos() as u32,
                pid: std::process::id() as u16,
                thread_id: thread::id() as u16,
                buffer_id: self.configuration_handle.getter().buffer_id,
                tag,
                priority,
                message: &message,
            };
            crate::logd::log(&log_record);
            if self.configuration_handle.getter().pstore {
                crate::pmsg::log(&log_record);
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            let now = ::time::OffsetDateTime::now_utc();
            let timestamp = now.format(&self.timestamp_format).unwrap();
            println!(
                "{} {} {} {} {}: {}",
                timestamp,
                std::process::id(),
                crate::thread::id(),
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
    fn flush(&self) {
        if self.configuration_handle.getter().pstore {
            crate::pmsg::flush().ok();
        }
    }
}

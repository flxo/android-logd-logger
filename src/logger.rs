use crate::{log_configuration::LogConfiguration, Buffer, Priority, TagMode};
#[cfg(target_os = "android")]
use crate::{thread, Record};
use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
use std::io;
use std::sync::{Arc, RwLock};
#[cfg(target_os = "android")]
use std::time::SystemTime;

///Logger configuration handler stores access to logger configuration parameters
///
///
#[derive(Clone)]
pub struct LoggerConfigHandler {
    pub(crate) configuration_handler: Arc<RwLock<LogConfiguration>>,
}

impl LoggerConfigHandler {
    /// Create new configuration handler from Arc<RwLock<LogConfiguration>>
    ///
    pub(crate) fn new(configuration: Arc<RwLock<LogConfiguration>>) -> Self {
        Self {
            configuration_handler: configuration,
        }
    }

    /// Create new configuration handler from LogConfiguration object
    ///     
    pub(crate) fn new_from_raw(configuration: LogConfiguration) -> Self {
        Self {
            configuration_handler: { Arc::new(RwLock::new(configuration)) },
        }
    }

    /// Provide access to configuration parameters for changing
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let config = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// config.setter().set_level_filter(LevelFilter::Error);
    /// ```
    pub fn setter(&self) -> std::sync::RwLockWriteGuard<LogConfiguration> {
        self.configuration_handler.write().unwrap()
    }

    /// Provide access to configuration parameters for reading
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let config = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// let prepend_module = config.getter().get_prepend_module();
    /// ```
    pub fn getter(&self) -> std::sync::RwLockReadGuard<LogConfiguration> {
        self.configuration_handler.read().unwrap()
    }

    pub(crate) fn get_config(&self) -> Arc<RwLock<LogConfiguration>> {
        Arc::clone(&self.configuration_handler)
    }
}
pub(crate) struct LoggerImpl {
    configuration_handle: LoggerConfigHandler,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl LoggerImpl {
    pub fn new(configuration: Arc<RwLock<LogConfiguration>>) -> Result<LoggerImpl, io::Error> {
        Ok(LoggerImpl {
            configuration_handle: LoggerConfigHandler::new(configuration),
            #[cfg(not(target_os = "android"))]
            timestamp_format: time::format_description::parse(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
            )
            .unwrap(),
        })
    }

    #[allow(unused)]
    pub fn level_filter(&self) -> LevelFilter {
        self.configuration_handle.getter().get_level_filter()
    }

    #[allow(unused)]
    pub fn set_filter(&mut self, new_filter: Filter) {
        self.configuration_handle.setter().set_filter(new_filter);
    }

    #[allow(unused)]
    pub fn set_tag(self, tag: &str) {
        self.configuration_handle.setter().set_custom_tag(tag);
    }

    #[allow(unused)]
    pub fn set_tag_target(&mut self) {
        self.configuration_handle.setter().set_tag_to_target();
    }

    #[allow(unused)]
    pub fn set_tag_target_strip(&mut self) {
        self.configuration_handle.setter().set_tag_to_target_strip();
    }

    #[allow(unused)]
    pub fn set_prepend_module(&mut self, new_prepend_module: bool) {
        self.configuration_handle.setter().set_prepend_module(new_prepend_module);
    }

    #[allow(unused)]
    pub fn set_buffer(&mut self, new_buffer: Buffer) {
        self.configuration_handle.setter().set_buffer(new_buffer);
    }
}

impl Log for LoggerImpl {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.configuration_handle.getter().filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.configuration_handle.getter().filter.matches(record) {
            return;
        }

        let args = record.args().to_string();
        let message = if let Some(module_path) = record.module_path() {
            if self.configuration_handle.getter().get_prepend_module() {
                [module_path, &args].join(": ")
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();
        let binding = self.configuration_handle.getter();
        let tag = match &binding.tag {
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
                buffer_id: self.buffer_id.into(),
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

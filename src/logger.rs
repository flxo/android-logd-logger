use crate::{configuration::Configuration, Buffer, Priority, TagMode};
#[cfg(target_os = "android")]
use crate::{thread, Record};
use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
#[cfg(target_os = "android")]
use std::time::SystemTime;
use std::{
    io,
    sync::{Arc, RwLock},
};

///Logger configuration handler stores access to logger configuration parameters
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
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_buffer(Buffer::Crash);
    /// ```
    pub fn set_buffer(&self, buffer: Buffer) -> &Self {
        self.configuration.write().unwrap().set_buffer(buffer);
        self
    }

    /// gets tag parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// let binder = logger.getter();
    /// ```
    pub fn getter(&self) -> std::sync::RwLockReadGuard<Configuration> {
        self.configuration.read().unwrap()
    }

    // Sets tag parameter of logger configuration to custom value
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_custom_tag("foo");
    /// ```
    pub fn set_custom_tag(&self, tag: &str) -> &Self {
        self.configuration.write().unwrap().set_custom_tag(tag);
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
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_tag_to_target();
    /// ```
    pub fn set_tag_to_target(&self) -> &Self {
        self.configuration.write().unwrap().set_tag_to_target();
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
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_tag_to_strip();
    /// ```
    pub fn set_tag_to_strip(&self) -> &Self {
        self.configuration.write().unwrap().set_tag_to_target_strip();
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
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .prepend_module(false)
    /// .init();
    ///
    /// logger.set_prepend_module(true);
    /// ```
    pub fn set_prepend_module(&self, prepend_module: bool) -> &Self {
        self.configuration.write().unwrap().set_prepend_module(prepend_module);
        self
    }

    /// Gets prepend module parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .prepend_module(false)
    /// .init();
    ///
    /// let prepend_module = logger.get_prepend_module();
    /// ```
    pub fn get_prepend_module(&self) -> bool {
        self.configuration.write().unwrap().prepend_module
    }

    /// Sets filter parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .pstore(false)
    /// .init();
    ///
    /// logger.set_pstore(true);
    /// ```
    #[cfg(target_os = "android")]
    pub fn set_pstore(&self, new_pstore: bool) -> &Self {
        self.logger.write().unwrap().set_pstore(new_pstore);
        self
    }

    /// Gets level filter parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// let level_filter = logger.get_level_filter();
    /// ```
    pub fn get_level_filter(&self) -> LevelFilter {
        self.configuration
            .write()
            .unwrap()
            .filter
            .filter()
            .to_level()
            .unwrap()
            .to_level_filter()
    }
    /// Sets filter parameter of logger configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// let mut filter_builder = env_logger::filter::Builder::default();
    /// let filter = filter_builder.filter_level(LevelFilter::Info).build();
    ///
    /// logger.set_filter(filter);
    /// ```
    pub fn set_filter(&self, filter: Filter) -> &Self {
        self.configuration.write().unwrap().set_filter(filter);
        self
    }

    /// Adds a directive to the filter for a specific module.
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_module_and_level_filter("path::to::module", LevelFilter::Info);
    /// ```
    pub fn set_module_and_level_filter(&self, module: &str, level: LevelFilter) -> &Self {
        self.configuration.write().unwrap().set_module_and_level_filter(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
    ///
    /// # Examples
    ///
    /// ```
    /// # use log::LevelFilter;
    /// # use android_logd_logger::Builder;
    ///
    /// let logger = android_logd_logger::builder()
    /// .parse_filters("debug")
    /// .init();
    ///
    /// logger.set_level_filter(LevelFilter::Info);
    /// ```
    pub fn set_level_filter(&self, level_filter: LevelFilter) -> &Self {
        self.configuration.write().unwrap().set_level_filter(level_filter);
        self
    }

    pub(crate) fn get_config(&self) -> Arc<RwLock<Configuration>> {
        Arc::clone(&self.configuration)
    }
}
pub(crate) struct LoggerImpl {
    configuration_handle: Logger,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl LoggerImpl {
    pub fn new(configuration: Arc<RwLock<Configuration>>) -> Result<LoggerImpl, io::Error> {
        Ok(LoggerImpl {
            configuration_handle: Logger { configuration },
            #[cfg(not(target_os = "android"))]
            timestamp_format: time::format_description::parse(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
            )
            .unwrap(),
        })
    }

    #[allow(unused)]
    pub fn set_filter(&mut self, new_filter: Filter) {
        self.configuration_handle.set_filter(new_filter);
    }

    #[allow(unused)]
    pub fn set_tag(self, tag: &str) {
        self.configuration_handle.set_custom_tag(tag);
    }

    #[allow(unused)]
    pub fn set_tag_target(&mut self) {
        self.configuration_handle.set_tag_to_target();
    }

    #[allow(unused)]
    pub fn set_tag_target_strip(&mut self) {
        self.configuration_handle.set_tag_to_strip();
    }

    #[allow(unused)]
    pub fn set_prepend_module(&mut self, new_prepend_module: bool) {
        self.configuration_handle.set_prepend_module(new_prepend_module);
    }

    #[allow(unused)]
    pub fn set_buffer(&mut self, new_buffer: Buffer) {
        self.configuration_handle.set_buffer(new_buffer);
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
            if self.configuration_handle.get_prepend_module() {
                [module_path, &args].join(": ")
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();
        let binder = self.configuration_handle.getter();
        let tag = match &binder.tag {
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

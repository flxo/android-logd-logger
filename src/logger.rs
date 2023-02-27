use crate::{log_configuration::LogConfiguration, Buffer, Priority, TagMode};
#[cfg(target_os = "android")]
use crate::{thread, Record};
use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
use std::io;
use std::sync::{Arc, RwLock};
#[cfg(target_os = "android")]
use std::time::SystemTime;

pub(crate) struct LoggerImpl {
    configuration: Arc<RwLock<LogConfiguration>>,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl LoggerImpl {
    pub fn new(configuration: Arc<RwLock<LogConfiguration>>) -> Result<LoggerImpl, io::Error> {
        Ok(LoggerImpl {
            configuration,
            #[cfg(not(target_os = "android"))]
            timestamp_format: time::format_description::parse(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
            )
            .unwrap(),
        })
    }

    pub fn level_filter(&self) -> LevelFilter {
        self.configuration.read().unwrap().filter.filter()
    }

    pub fn get_logger_handler(&self) {}

    pub fn set_filter(&mut self, new_filter: Filter) {
        self.configuration.write().unwrap().set_filter(new_filter);
    }

    pub fn set_tag(self, tag: &str) {
        self.configuration.write().unwrap().tag(tag);
    }

    pub fn set_tag_target(&mut self) {
        self.configuration.write().unwrap().tag_target();
    }

    pub fn set_tag_target_strip(&mut self) {
        self.configuration.write().unwrap().tag_target_strip();
    }

    pub fn set_prepend_module(&mut self, new_prepend_module: bool) {
        self.configuration.write().unwrap().prepend_module(new_prepend_module);
    }

    pub fn set_buffer(&mut self, new_buffer: Buffer) {
        self.configuration.write().unwrap().buffer(new_buffer);
    }
}

impl Log for LoggerImpl {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.configuration.read().unwrap().filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.configuration.read().unwrap().filter.matches(record) {
            return;
        }

        let args = record.args().to_string();
        let message = if let Some(module_path) = record.module_path() {
            if self.configuration.read().unwrap().prepend_module {
                [module_path, &args].join(": ")
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();
        let binding = self.configuration.read().unwrap();
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
            if self.configuration.read().unwrap().pstore {
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
        if self.configuration.read().unwrap().pstore {
            crate::pmsg::flush().ok();
        }
    }
}

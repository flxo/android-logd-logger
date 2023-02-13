#[cfg(target_os = "android")]
use crate::{thread, Record};
use crate::{Buffer, Priority, TagMode};
use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
use std::io;
#[cfg(target_os = "android")]
use std::time::SystemTime;

pub(crate) struct Logger {
    filter: Filter,
    tag: TagMode,
    prepend_module: bool,
    #[allow(unused)]
    pstore: bool,
    #[allow(unused)]
    buffer_id: Buffer,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl Logger {
    pub fn new(buffer_id: Buffer, filter: Filter, tag: TagMode, prepend_module: bool, pstore: bool) -> Result<Logger, io::Error> {
        Ok(Logger {
            filter,
            tag,
            prepend_module,
            pstore,
            buffer_id,
            #[cfg(not(target_os = "android"))]
            timestamp_format: time::format_description::parse(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]",
            )
            .unwrap(),
        })
    }

    pub fn level_filter(&self) -> LevelFilter {
        self.filter.filter()
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
                [module_path, &args].join(": ")
            } else {
                args
            }
        } else {
            args
        };

        let priority: Priority = record.metadata().level().into();
        let tag = match &self.tag {
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
            let log_record = Record {
                timestamp: SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("failed to acquire time"),
                pid: std::process::id() as u16,
                thread_id: thread::id() as u16,
                buffer_id: self.buffer_id.into(),
                tag,
                priority,
                message: &message,
            };
            crate::logd::log(&log_record);
            if self.pstore {
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
        if self.pstore {
            crate::pmsg::flush().ok();
        }
    }
}

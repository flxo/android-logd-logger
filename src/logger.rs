use env_logger::filter::Filter;
use log::{LevelFilter, Log, Metadata};
use std::io;

use crate::{Buffer, Priority, TagMode};

pub(crate) struct Logger {
    filter: Filter,
    tag: TagMode,
    prepend_module: bool,
    #[allow(unused)]
    persistent_logging: bool,
    #[allow(unused)]
    buffer_id: Buffer,

    #[cfg(not(target_os = "android"))]
    timestamp_format: Vec<time::format_description::FormatItem<'static>>,
}

impl Logger {
    pub fn new(
        buffer_id: Buffer,
        filter: Filter,
        tag: TagMode,
        prepend_module: bool,
        persistent_logging: bool,
    ) -> Result<Logger, io::Error> {
        Ok(Logger {
            filter,
            tag,
            prepend_module,
            persistent_logging,
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
            crate::logd::log(tag, self.buffer_id, priority, &message);
            if self.persistent_logging {
                crate::pmsg::android::log(tag, self.buffer_id, priority, &message);
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
        if self.persistent_logging {
            crate::pmsg::android::flush().ok();
        }
    }
}

use super::{Buffer, TagMode};

use env_logger::filter::Filter;

/// Documentation here
pub struct LogConfiguration {
    pub(crate) filter: Filter,
    pub(crate) tag: TagMode,
    pub(crate) prepend_module: bool,
    #[allow(unused)]
    pub(crate) pstore: bool,
    pub(crate) buffer_id: Option<Buffer>,
}

impl LogConfiguration {
    /// Initializes the Log Configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// ```
    ///
    /// [`filter`]: #method.filter
    pub(crate) fn new(filter: Filter, tag: TagMode, prepend_module: bool, pstore: bool, buffer_id: Option<Buffer>) -> Self {
        //let tag = TagMode::default();
        Self {
            filter,
            tag,
            prepend_module,
            pstore,
            buffer_id,
        }
    }

    /// Set Log configuration buffer
    ///
    /// # Examples
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.buffer(Buffer::Main);
    ///
    pub fn buffer(&mut self, buffer: Buffer) -> &mut Self {
        self.buffer_id = Some(buffer);
        self
    }

    /// Set Log configuration custom tag
    ///
    /// # Examples
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.tag("custom tag");
    ///
    pub fn tag(&mut self, tag: &str) -> &mut Self {
        self.tag = TagMode::Custom(tag.to_string());
        self
    }

    /// Set Log configuration tag by target
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.tag("custom tag");
    ///
    pub fn tag_target(&mut self) -> &mut Self {
        self.tag = TagMode::Target;
        self
    }

    /// Set Log configuration tag by target
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.tag("custom tag");
    ///
    pub fn tag_target_strip(&mut self) -> &mut Self {
        self.tag = TagMode::TargetStrip;
        self
    }

    /// Set Log configuration prepend module
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.prepend_module(true);
    ///
    pub fn prepend_module(&mut self, prepend_module: bool) -> &mut Self {
        self.prepend_module = prepend_module;
        self
    }

    /// Set Log configuration pstore
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.set_pstore(true);
    ///
    pub fn set_pstore(&mut self, new_pstore: bool) -> &mut Self {
        self.pstore = new_pstore;
        self
    }

    /// Set Log configuration pstore
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.set_filter(filter);
    ///
    pub fn set_filter(&mut self, filter: Filter) -> &mut Self {
        self.filter = filter;
        self
    }

    /// Set Log configuration pstore
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.set_level_filter(log::LevelFilter::Error);
    ///
    pub fn set_level_filter(&mut self, level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_level(level_filter).build();
        self.set_filter(filter);
        self
    }

    /// Set Log configuration pstore
    ///
    /// # Examples
    ///
    /// Create a new builder and configure filters and style:
    ///
    /// ```
    /// # use env_logger::filter::Filter;
    /// # use android_logd_logger::{LogConfiguration, Builder};
    /// let mut builder = Builder::new();
    ///
    /// builder.buffer(Buffer::Crash).init();
    /// builder.tag_target().init();
    /// builder.filter_module("path::to::module", LevelFilter::Info).init();
    ///
    /// let buffer = builder.buffer.unwrap_or(Buffer::Main);
    /// let filter = builder.filter.build();
    /// let tag = builder.tag.clone();
    ///
    /// let mut log_config = LogConfiguration::new(filter, tag, false, false, Some(buffer));
    ///
    /// log_config.set_mudule_and_level_filter("module", log::LevelFilter::Error);
    ///
    pub fn set_mudule_and_level_filter(&mut self, new_module: &str, new_level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_module(new_module, new_level_filter).build();
        self.set_filter(filter);
        self
    }
}

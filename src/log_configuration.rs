use super::{Buffer, TagMode};

use env_logger::filter::Filter;

/// Contain all logger properties
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
    pub(crate) fn new(filter: Filter, tag: TagMode, prepend_module: bool, pstore: bool, buffer_id: Option<Buffer>) -> Self {
        Self {
            filter,
            tag,
            prepend_module,
            pstore,
            buffer_id,
        }
    }

    /// Set Log configuration buffer field
    pub fn set_buffer(&mut self, buffer: Buffer) -> &mut Self {
        self.buffer_id = Some(buffer);
        self
    }

    /// Set Log configuration tag to custom
    pub fn set_custom_tag(&mut self, tag: &str) -> &mut Self {
        self.tag = TagMode::Custom(tag.to_string());
        self
    }

    /// Set Log configuration tag to target
    pub fn set_tag_to_target(&mut self) -> &mut Self {
        self.tag = TagMode::Target;
        self
    }

    /// Set Log configuration tag to target strip
    pub fn set_tag_to_target_strip(&mut self) -> &mut Self {
        self.tag = TagMode::TargetStrip;
        self
    }

    /// Set Log configuration prepend module
    pub fn set_prepend_module(&mut self, prepend_module: bool) -> &mut Self {
        self.prepend_module = prepend_module;
        self
    }

    /// Set Log configuration pstore
    pub fn set_pstore(&mut self, new_pstore: bool) -> &mut Self {
        self.pstore = new_pstore;
        self
    }

    /// Set Log configuration filter
    pub fn set_filter(&mut self, filter: Filter) -> &mut Self {
        self.filter = filter;
        self
    }

    /// Set Log configuration filter level
    pub fn set_level_filter(&mut self, level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_level(level_filter).build();
        self.set_filter(filter);
        self
    }

    /// Set Log configuration module and level filter
    pub fn set_module_and_level_filter(&mut self, new_module: &str, new_level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_module(new_module, new_level_filter).build();
        self.set_filter(filter);
        self
    }
}

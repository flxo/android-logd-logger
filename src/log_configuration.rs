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
    /// Documentation here
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

    /// Documentation here
    pub fn buffer(&mut self, buffer: Buffer) -> &mut Self {
        self.buffer_id = Some(buffer);
        self
    }

    /// Documentation here
    pub fn tag(&mut self, tag: &str) -> &mut Self {
        self.tag = TagMode::Custom(tag.to_string());
        self
    }

    /// Documentation here
    pub fn tag_target(&mut self) -> &mut Self {
        self.tag = TagMode::Target;
        self
    }

    /// Documentation here
    pub fn tag_target_strip(&mut self) -> &mut Self {
        self.tag = TagMode::TargetStrip;
        self
    }

    /// Documentation here
    pub fn prepend_module(&mut self, prepend_module: bool) -> &mut Self {
        self.prepend_module = prepend_module;
        self
    }

    /// Documentation here
    pub fn set_pstore(&mut self, new_pstore: bool) -> &mut Self {
        self.pstore = new_pstore;
        self
    }

    /// Documentation here
    pub fn set_filter(&mut self, filter: Filter) -> &mut Self {
        self.filter = filter;
        self
    }

    /// Documentation here
    pub fn set_level_filter(&mut self, level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_level(level_filter).build();
        self.set_filter(filter);
        self
    }

    /// Documentation here
    pub fn set_mudule_and_level_filter(&mut self, new_module: &str, new_level_filter: log::LevelFilter) -> &mut Self {
        let mut filter_builder = env_logger::filter::Builder::default();
        let filter = filter_builder.filter_module(new_module, new_level_filter).build();
        self.set_filter(filter);
        self
    }
}

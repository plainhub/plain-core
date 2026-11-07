use std::cell::Cell;

thread_local! {
    static GLOBAL_OPTIONS: Cell<GlobalOptions> = Cell::new(GlobalOptions::default());
}

pub(crate) fn global_options() -> GlobalOptions {
    GLOBAL_OPTIONS.with(Cell::get)
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[non_exhaustive]
pub struct GlobalOptions {
    pub(crate) use_custom_resolvers: bool,
    pub(crate) allocation_limit: usize,
    pub(crate) preserve_format_specific_items: bool,
}

impl GlobalOptions {
    pub const DEFAULT_ALLOCATION_LIMIT: usize = 16 * 1024 * 1024;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            use_custom_resolvers: true,
            allocation_limit: Self::DEFAULT_ALLOCATION_LIMIT,
            preserve_format_specific_items: true,
        }
    }

    pub fn use_custom_resolvers(&mut self, use_custom_resolvers: bool) -> Self {
        self.use_custom_resolvers = use_custom_resolvers;
        *self
    }

    pub fn allocation_limit(&mut self, allocation_limit: usize) -> Self {
        self.allocation_limit = allocation_limit;
        *self
    }

    pub fn preserve_format_specific_items(&mut self, preserve_format_specific_items: bool) -> Self {
        self.preserve_format_specific_items = preserve_format_specific_items;
        *self
    }
}

impl Default for GlobalOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub fn apply_global_options(options: GlobalOptions) {
    GLOBAL_OPTIONS.with(|global_options| global_options.set(options));
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/config/global_options.rs"]
mod snapshot_tests;

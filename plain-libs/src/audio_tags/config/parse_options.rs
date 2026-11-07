#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ParseOptions {
    pub(crate) read_properties: bool,
    pub(crate) read_tags: bool,
    pub(crate) parsing_mode: ParsingMode,
    pub(crate) max_junk_bytes: usize,
    pub(crate) read_cover_art: bool,
    pub(crate) implicit_conversions: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseOptions {
    pub const DEFAULT_PARSING_MODE: ParsingMode = ParsingMode::BestAttempt;

    pub const DEFAULT_MAX_JUNK_BYTES: usize = 1024;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            read_properties: true,
            read_tags: true,
            parsing_mode: Self::DEFAULT_PARSING_MODE,
            max_junk_bytes: Self::DEFAULT_MAX_JUNK_BYTES,
            read_cover_art: true,
            implicit_conversions: true,
        }
    }

    pub fn read_properties(&mut self, read_properties: bool) -> Self {
        self.read_properties = read_properties;
        *self
    }

    pub fn read_tags(&mut self, read_tags: bool) -> Self {
        self.read_tags = read_tags;
        *self
    }

    pub fn parsing_mode(&mut self, parsing_mode: ParsingMode) -> Self {
        self.parsing_mode = parsing_mode;
        *self
    }

    pub fn max_junk_bytes(&mut self, max_junk_bytes: usize) -> Self {
        self.max_junk_bytes = max_junk_bytes;
        *self
    }

    pub fn read_cover_art(&mut self, read_cover_art: bool) -> Self {
        self.read_cover_art = read_cover_art;
        *self
    }

    pub fn implicit_conversions(&mut self, implicit_conversions: bool) -> Self {
        self.implicit_conversions = implicit_conversions;
        *self
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Default)]
#[non_exhaustive]
pub enum ParsingMode {
    Strict,

    #[default]
    BestAttempt,

    Relaxed,
}

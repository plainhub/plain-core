use crate::regex_engine::syntax::{
    Error, ParserBuilder, ast,
    hir::{self, Hir},
};
use std::{vec, vec::Vec};
pub fn parse(pattern: &str) -> Result<Hir, Error> {
    parse_with(pattern, &Config::default())
}

pub fn parse_with(pattern: &str, config: &Config) -> Result<Hir, Error> {
    let mut builder = ParserBuilder::new();
    config.apply(&mut builder);
    builder.build().parse(pattern)
}
pub fn parse_many_with<P: AsRef<str>>(patterns: &[P], config: &Config) -> Result<Vec<Hir>, Error> {
    let mut builder = ParserBuilder::new();
    config.apply(&mut builder);
    let mut hirs = vec![];
    for p in patterns.iter() {
        hirs.push(builder.build().parse(p.as_ref())?);
    }
    Ok(hirs)
}
#[derive(Clone, Copy, Debug)]
pub struct Config {
    case_insensitive: bool,
    multi_line: bool,
    dot_matches_new_line: bool,
    crlf: bool,
    line_terminator: u8,
    swap_greed: bool,
    ignore_whitespace: bool,
    unicode: bool,
    utf8: bool,
    nest_limit: u32,
    octal: bool,
}
impl Config {
    pub fn new() -> Config {
        Config {
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
            crlf: false,
            line_terminator: b'\n',
            swap_greed: false,
            ignore_whitespace: false,
            unicode: true,
            utf8: true,
            nest_limit: 250,
            octal: false,
        }
    }
    pub fn case_insensitive(mut self, yes: bool) -> Config {
        self.case_insensitive = yes;
        self
    }
    pub fn multi_line(mut self, yes: bool) -> Config {
        self.multi_line = yes;
        self
    }
    pub fn dot_matches_new_line(mut self, yes: bool) -> Config {
        self.dot_matches_new_line = yes;
        self
    }
    pub fn crlf(mut self, yes: bool) -> Config {
        self.crlf = yes;
        self
    }
    pub fn line_terminator(mut self, byte: u8) -> Config {
        self.line_terminator = byte;
        self
    }
    pub fn swap_greed(mut self, yes: bool) -> Config {
        self.swap_greed = yes;
        self
    }
    pub fn ignore_whitespace(mut self, yes: bool) -> Config {
        self.ignore_whitespace = yes;
        self
    }
    pub fn unicode(mut self, yes: bool) -> Config {
        self.unicode = yes;
        self
    }
    pub fn utf8(mut self, yes: bool) -> Config {
        self.utf8 = yes;
        self
    }
    pub fn nest_limit(mut self, limit: u32) -> Config {
        self.nest_limit = limit;
        self
    }
    pub fn octal(mut self, yes: bool) -> Config {
        self.octal = yes;
        self
    }

    pub fn get_line_terminator(&self) -> u8 {
        self.line_terminator
    }

    pub fn get_utf8(&self) -> bool {
        self.utf8
    }

    pub(crate) fn apply(&self, builder: &mut ParserBuilder) {
        builder
            .unicode(self.unicode)
            .case_insensitive(self.case_insensitive)
            .multi_line(self.multi_line)
            .dot_matches_new_line(self.dot_matches_new_line)
            .crlf(self.crlf)
            .line_terminator(self.line_terminator)
            .swap_greed(self.swap_greed)
            .ignore_whitespace(self.ignore_whitespace)
            .utf8(self.utf8)
            .nest_limit(self.nest_limit)
            .octal(self.octal);
    }
    pub(crate) fn apply_ast(&self, builder: &mut ast::parse::ParserBuilder) {
        builder
            .ignore_whitespace(self.ignore_whitespace)
            .nest_limit(self.nest_limit)
            .octal(self.octal);
    }
    pub(crate) fn apply_hir(&self, builder: &mut hir::translate::TranslatorBuilder) {
        builder
            .unicode(self.unicode)
            .case_insensitive(self.case_insensitive)
            .multi_line(self.multi_line)
            .crlf(self.crlf)
            .dot_matches_new_line(self.dot_matches_new_line)
            .line_terminator(self.line_terminator)
            .swap_greed(self.swap_greed)
            .utf8(self.utf8);
    }
}
impl Default for Config {
    fn default() -> Config {
        Config::new()
    }
}

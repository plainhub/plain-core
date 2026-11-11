use crate::regex_engine::syntax::{Error, ast, hir};
pub fn parse(pattern: &str) -> Result<hir::Hir, Error> {
    Parser::new().parse(pattern)
}
#[derive(Clone, Debug, Default)]
pub struct ParserBuilder {
    ast: ast::parse::ParserBuilder,
    hir: hir::translate::TranslatorBuilder,
}
impl ParserBuilder {
    pub fn new() -> ParserBuilder {
        ParserBuilder::default()
    }
    pub fn build(&self) -> Parser {
        Parser {
            ast: self.ast.build(),
            hir: self.hir.build(),
        }
    }
    pub fn nest_limit(&mut self, limit: u32) -> &mut ParserBuilder {
        self.ast.nest_limit(limit);
        self
    }
    pub fn octal(&mut self, yes: bool) -> &mut ParserBuilder {
        self.ast.octal(yes);
        self
    }
    pub fn utf8(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.utf8(yes);
        self
    }
    pub fn ignore_whitespace(&mut self, yes: bool) -> &mut ParserBuilder {
        self.ast.ignore_whitespace(yes);
        self
    }
    pub fn case_insensitive(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.case_insensitive(yes);
        self
    }
    pub fn multi_line(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.multi_line(yes);
        self
    }
    pub fn dot_matches_new_line(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.dot_matches_new_line(yes);
        self
    }
    pub fn crlf(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.crlf(yes);
        self
    }
    pub fn line_terminator(&mut self, byte: u8) -> &mut ParserBuilder {
        self.hir.line_terminator(byte);
        self
    }
    pub fn swap_greed(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.swap_greed(yes);
        self
    }
    pub fn unicode(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.unicode(yes);
        self
    }
}
#[derive(Clone, Debug)]
pub struct Parser {
    ast: ast::parse::Parser,
    hir: hir::translate::Translator,
}
impl Parser {
    pub fn new() -> Parser {
        ParserBuilder::new().build()
    }
    pub fn parse(&mut self, pattern: &str) -> Result<hir::Hir, Error> {
        let ast = self.ast.parse(pattern)?;
        let hir = self.hir.translate(pattern, &ast)?;
        Ok(hir)
    }
}

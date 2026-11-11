use crate::regex_engine::syntax::ast::{
    self, Ast,
    visitor::{self, Visitor},
};
use core::fmt;
#[derive(Clone, Debug)]
struct PrinterBuilder {
    _priv: (),
}
impl Default for PrinterBuilder {
    fn default() -> PrinterBuilder {
        PrinterBuilder::new()
    }
}
impl PrinterBuilder {
    fn new() -> PrinterBuilder {
        PrinterBuilder { _priv: () }
    }
    fn build(&self) -> Printer {
        Printer { _priv: () }
    }
}
#[derive(Debug)]
pub struct Printer {
    _priv: (),
}
impl Printer {
    pub fn new() -> Printer {
        PrinterBuilder::new().build()
    }
    pub fn print<W: fmt::Write>(&mut self, ast: &Ast, wtr: W) -> fmt::Result {
        visitor::visit(ast, Writer { wtr })
    }
}
#[derive(Debug)]
struct Writer<W> {
    wtr: W,
}
impl<W: fmt::Write> Visitor for Writer<W> {
    type Output = ();
    type Err = fmt::Error;
    fn finish(self) -> fmt::Result {
        Ok(())
    }
    fn visit_pre(&mut self, ast: &Ast) -> fmt::Result {
        match *ast {
            Ast::Group(ref x) => self.fmt_group_pre(x),
            Ast::ClassBracketed(ref x) => self.fmt_class_bracketed_pre(x),
            _ => Ok(()),
        }
    }
    fn visit_post(&mut self, ast: &Ast) -> fmt::Result {
        match *ast {
            Ast::Empty(_) => Ok(()),
            Ast::Flags(ref x) => self.fmt_set_flags(x),
            Ast::Literal(ref x) => self.fmt_literal(x),
            Ast::Dot(_) => self.wtr.write_str("."),
            Ast::Assertion(ref x) => self.fmt_assertion(x),
            Ast::ClassPerl(ref x) => self.fmt_class_perl(x),
            Ast::ClassUnicode(ref x) => self.fmt_class_unicode(x),
            Ast::ClassBracketed(ref x) => self.fmt_class_bracketed_post(x),
            Ast::Repetition(ref x) => self.fmt_repetition(x),
            Ast::Group(ref x) => self.fmt_group_post(x),
            Ast::Alternation(_) => Ok(()),
            Ast::Concat(_) => Ok(()),
        }
    }
    fn visit_alternation_in(&mut self) -> fmt::Result {
        self.wtr.write_str("|")
    }
    fn visit_class_set_item_pre(&mut self, ast: &ast::ClassSetItem) -> Result<(), Self::Err> {
        match *ast {
            ast::ClassSetItem::Bracketed(ref x) => self.fmt_class_bracketed_pre(x),
            _ => Ok(()),
        }
    }
    fn visit_class_set_item_post(&mut self, ast: &ast::ClassSetItem) -> Result<(), Self::Err> {
        use crate::regex_engine::syntax::ast::ClassSetItem::*;
        match *ast {
            Empty(_) => Ok(()),
            Literal(ref x) => self.fmt_literal(x),
            Range(ref x) => {
                self.fmt_literal(&x.start)?;
                self.wtr.write_str("-")?;
                self.fmt_literal(&x.end)?;
                Ok(())
            }
            Ascii(ref x) => self.fmt_class_ascii(x),
            Unicode(ref x) => self.fmt_class_unicode(x),
            Perl(ref x) => self.fmt_class_perl(x),
            Bracketed(ref x) => self.fmt_class_bracketed_post(x),
            Union(_) => Ok(()),
        }
    }
    fn visit_class_set_binary_op_in(
        &mut self,
        ast: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        self.fmt_class_set_binary_op_kind(&ast.kind)
    }
}
impl<W: fmt::Write> Writer<W> {
    fn fmt_group_pre(&mut self, ast: &ast::Group) -> fmt::Result {
        use crate::regex_engine::syntax::ast::GroupKind::*;
        match ast.kind {
            CaptureIndex(_) => self.wtr.write_str("("),
            CaptureName {
                ref name,
                starts_with_p,
            } => {
                let start = if starts_with_p { "(?P<" } else { "(?<" };
                self.wtr.write_str(start)?;
                self.wtr.write_str(&name.name)?;
                self.wtr.write_str(">")?;
                Ok(())
            }
            NonCapturing(ref flags) => {
                self.wtr.write_str("(?")?;
                self.fmt_flags(flags)?;
                self.wtr.write_str(":")?;
                Ok(())
            }
        }
    }
    fn fmt_group_post(&mut self, _ast: &ast::Group) -> fmt::Result {
        self.wtr.write_str(")")
    }
    fn fmt_repetition(&mut self, ast: &ast::Repetition) -> fmt::Result {
        use crate::regex_engine::syntax::ast::RepetitionKind::*;
        match ast.op.kind {
            ZeroOrOne if ast.greedy => self.wtr.write_str("?"),
            ZeroOrOne => self.wtr.write_str("??"),
            ZeroOrMore if ast.greedy => self.wtr.write_str("*"),
            ZeroOrMore => self.wtr.write_str("*?"),
            OneOrMore if ast.greedy => self.wtr.write_str("+"),
            OneOrMore => self.wtr.write_str("+?"),
            Range(ref x) => {
                self.fmt_repetition_range(x)?;
                if !ast.greedy {
                    self.wtr.write_str("?")?;
                }
                Ok(())
            }
        }
    }
    fn fmt_repetition_range(&mut self, ast: &ast::RepetitionRange) -> fmt::Result {
        use crate::regex_engine::syntax::ast::RepetitionRange::*;
        match *ast {
            Exactly(x) => write!(self.wtr, "{{{x}}}"),
            AtLeast(x) => write!(self.wtr, "{{{x},}}"),
            Bounded(x, y) => write!(self.wtr, "{{{x},{y}}}"),
        }
    }
    fn fmt_literal(&mut self, ast: &ast::Literal) -> fmt::Result {
        use crate::regex_engine::syntax::ast::LiteralKind::*;
        match ast.kind {
            Verbatim => self.wtr.write_char(ast.c),
            Meta | Superfluous => write!(self.wtr, r"\{}", ast.c),
            Octal => write!(self.wtr, r"\{:o}", u32::from(ast.c)),
            HexFixed(ast::HexLiteralKind::X) => {
                write!(self.wtr, r"\x{:02X}", u32::from(ast.c))
            }
            HexFixed(ast::HexLiteralKind::UnicodeShort) => {
                write!(self.wtr, r"\u{:04X}", u32::from(ast.c))
            }
            HexFixed(ast::HexLiteralKind::UnicodeLong) => {
                write!(self.wtr, r"\U{:08X}", u32::from(ast.c))
            }
            HexBrace(ast::HexLiteralKind::X) => {
                write!(self.wtr, r"\x{{{:X}}}", u32::from(ast.c))
            }
            HexBrace(ast::HexLiteralKind::UnicodeShort) => {
                write!(self.wtr, r"\u{{{:X}}}", u32::from(ast.c))
            }
            HexBrace(ast::HexLiteralKind::UnicodeLong) => {
                write!(self.wtr, r"\U{{{:X}}}", u32::from(ast.c))
            }
            Special(ast::SpecialLiteralKind::Bell) => self.wtr.write_str(r"\a"),
            Special(ast::SpecialLiteralKind::FormFeed) => self.wtr.write_str(r"\f"),
            Special(ast::SpecialLiteralKind::Tab) => self.wtr.write_str(r"\t"),
            Special(ast::SpecialLiteralKind::LineFeed) => self.wtr.write_str(r"\n"),
            Special(ast::SpecialLiteralKind::CarriageReturn) => self.wtr.write_str(r"\r"),
            Special(ast::SpecialLiteralKind::VerticalTab) => self.wtr.write_str(r"\v"),
            Special(ast::SpecialLiteralKind::Space) => self.wtr.write_str(r"\ "),
        }
    }
    fn fmt_assertion(&mut self, ast: &ast::Assertion) -> fmt::Result {
        use crate::regex_engine::syntax::ast::AssertionKind::*;
        match ast.kind {
            StartLine => self.wtr.write_str("^"),
            EndLine => self.wtr.write_str("$"),
            StartText => self.wtr.write_str(r"\A"),
            EndText => self.wtr.write_str(r"\z"),
            WordBoundary => self.wtr.write_str(r"\b"),
            NotWordBoundary => self.wtr.write_str(r"\B"),
            WordBoundaryStart => self.wtr.write_str(r"\b{start}"),
            WordBoundaryEnd => self.wtr.write_str(r"\b{end}"),
            WordBoundaryStartAngle => self.wtr.write_str(r"\<"),
            WordBoundaryEndAngle => self.wtr.write_str(r"\>"),
            WordBoundaryStartHalf => self.wtr.write_str(r"\b{start-half}"),
            WordBoundaryEndHalf => self.wtr.write_str(r"\b{end-half}"),
        }
    }
    fn fmt_set_flags(&mut self, ast: &ast::SetFlags) -> fmt::Result {
        self.wtr.write_str("(?")?;
        self.fmt_flags(&ast.flags)?;
        self.wtr.write_str(")")?;
        Ok(())
    }
    fn fmt_flags(&mut self, ast: &ast::Flags) -> fmt::Result {
        use crate::regex_engine::syntax::ast::{Flag, FlagsItemKind};
        for item in &ast.items {
            match item.kind {
                FlagsItemKind::Negation => self.wtr.write_str("-"),
                FlagsItemKind::Flag(ref flag) => match *flag {
                    Flag::CaseInsensitive => self.wtr.write_str("i"),
                    Flag::MultiLine => self.wtr.write_str("m"),
                    Flag::DotMatchesNewLine => self.wtr.write_str("s"),
                    Flag::SwapGreed => self.wtr.write_str("U"),
                    Flag::Unicode => self.wtr.write_str("u"),
                    Flag::CRLF => self.wtr.write_str("R"),
                    Flag::IgnoreWhitespace => self.wtr.write_str("x"),
                },
            }?;
        }
        Ok(())
    }
    fn fmt_class_bracketed_pre(&mut self, ast: &ast::ClassBracketed) -> fmt::Result {
        if ast.negated {
            self.wtr.write_str("[^")
        } else {
            self.wtr.write_str("[")
        }
    }
    fn fmt_class_bracketed_post(&mut self, _ast: &ast::ClassBracketed) -> fmt::Result {
        self.wtr.write_str("]")
    }
    fn fmt_class_set_binary_op_kind(&mut self, ast: &ast::ClassSetBinaryOpKind) -> fmt::Result {
        use crate::regex_engine::syntax::ast::ClassSetBinaryOpKind::*;
        match *ast {
            Intersection => self.wtr.write_str("&&"),
            Difference => self.wtr.write_str("--"),
            SymmetricDifference => self.wtr.write_str("~~"),
        }
    }
    fn fmt_class_perl(&mut self, ast: &ast::ClassPerl) -> fmt::Result {
        use crate::regex_engine::syntax::ast::ClassPerlKind::*;
        match ast.kind {
            Digit if ast.negated => self.wtr.write_str(r"\D"),
            Digit => self.wtr.write_str(r"\d"),
            Space if ast.negated => self.wtr.write_str(r"\S"),
            Space => self.wtr.write_str(r"\s"),
            Word if ast.negated => self.wtr.write_str(r"\W"),
            Word => self.wtr.write_str(r"\w"),
        }
    }
    fn fmt_class_ascii(&mut self, ast: &ast::ClassAscii) -> fmt::Result {
        use crate::regex_engine::syntax::ast::ClassAsciiKind::*;
        match ast.kind {
            Alnum if ast.negated => self.wtr.write_str("[:^alnum:]"),
            Alnum => self.wtr.write_str("[:alnum:]"),
            Alpha if ast.negated => self.wtr.write_str("[:^alpha:]"),
            Alpha => self.wtr.write_str("[:alpha:]"),
            Ascii if ast.negated => self.wtr.write_str("[:^ascii:]"),
            Ascii => self.wtr.write_str("[:ascii:]"),
            Blank if ast.negated => self.wtr.write_str("[:^blank:]"),
            Blank => self.wtr.write_str("[:blank:]"),
            Cntrl if ast.negated => self.wtr.write_str("[:^cntrl:]"),
            Cntrl => self.wtr.write_str("[:cntrl:]"),
            Digit if ast.negated => self.wtr.write_str("[:^digit:]"),
            Digit => self.wtr.write_str("[:digit:]"),
            Graph if ast.negated => self.wtr.write_str("[:^graph:]"),
            Graph => self.wtr.write_str("[:graph:]"),
            Lower if ast.negated => self.wtr.write_str("[:^lower:]"),
            Lower => self.wtr.write_str("[:lower:]"),
            Print if ast.negated => self.wtr.write_str("[:^print:]"),
            Print => self.wtr.write_str("[:print:]"),
            Punct if ast.negated => self.wtr.write_str("[:^punct:]"),
            Punct => self.wtr.write_str("[:punct:]"),
            Space if ast.negated => self.wtr.write_str("[:^space:]"),
            Space => self.wtr.write_str("[:space:]"),
            Upper if ast.negated => self.wtr.write_str("[:^upper:]"),
            Upper => self.wtr.write_str("[:upper:]"),
            Word if ast.negated => self.wtr.write_str("[:^word:]"),
            Word => self.wtr.write_str("[:word:]"),
            Xdigit if ast.negated => self.wtr.write_str("[:^xdigit:]"),
            Xdigit => self.wtr.write_str("[:xdigit:]"),
        }
    }
    fn fmt_class_unicode(&mut self, ast: &ast::ClassUnicode) -> fmt::Result {
        use crate::regex_engine::syntax::ast::ClassUnicodeKind::*;
        use crate::regex_engine::syntax::ast::ClassUnicodeOpKind::*;
        if ast.negated {
            self.wtr.write_str(r"\P")?;
        } else {
            self.wtr.write_str(r"\p")?;
        }
        match ast.kind {
            OneLetter(c) => self.wtr.write_char(c),
            Named(ref x) => write!(self.wtr, "{{{}}}", x),
            NamedValue {
                op: Equal,
                ref name,
                ref value,
            } => {
                write!(self.wtr, "{{{}={}}}", name, value)
            }
            NamedValue {
                op: Colon,
                ref name,
                ref value,
            } => {
                write!(self.wtr, "{{{}:{}}}", name, value)
            }
            NamedValue {
                op: NotEqual,
                ref name,
                ref value,
            } => {
                write!(self.wtr, "{{{}!={}}}", name, value)
            }
        }
    }
}

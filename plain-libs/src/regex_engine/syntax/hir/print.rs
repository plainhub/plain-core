use crate::regex_engine::syntax::{
    hir::{
        self, Hir, HirKind,
        visitor::{self, Visitor},
    },
    is_meta_character,
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
    pub fn print<W: fmt::Write>(&mut self, hir: &Hir, wtr: W) -> fmt::Result {
        visitor::visit(hir, Writer { wtr })
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
    fn visit_pre(&mut self, hir: &Hir) -> fmt::Result {
        match *hir.kind() {
            HirKind::Empty => {
                self.wtr.write_str(r"(?:)")?;
            }
            HirKind::Repetition(_) => {}
            HirKind::Literal(hir::Literal(ref bytes)) => {
                let result = core::str::from_utf8(bytes);
                let len = result.map_or(bytes.len(), |s| s.chars().count());
                if len > 1 {
                    self.wtr.write_str(r"(?:")?;
                }
                match result {
                    Ok(string) => {
                        for c in string.chars() {
                            self.write_literal_char(c)?;
                        }
                    }
                    Err(_) => {
                        for &b in bytes.iter() {
                            self.write_literal_byte(b)?;
                        }
                    }
                }
                if len > 1 {
                    self.wtr.write_str(r")")?;
                }
            }
            HirKind::Class(hir::Class::Unicode(ref cls)) => {
                if cls.ranges().is_empty() {
                    return self.wtr.write_str("[a&&b]");
                }
                self.wtr.write_str("[")?;
                for range in cls.iter() {
                    if range.start() == range.end() {
                        self.write_literal_char(range.start())?;
                    } else if u32::from(range.start()) + 1 == u32::from(range.end()) {
                        self.write_literal_char(range.start())?;
                        self.write_literal_char(range.end())?;
                    } else {
                        self.write_literal_char(range.start())?;
                        self.wtr.write_str("-")?;
                        self.write_literal_char(range.end())?;
                    }
                }
                self.wtr.write_str("]")?;
            }
            HirKind::Class(hir::Class::Bytes(ref cls)) => {
                if cls.ranges().is_empty() {
                    return self.wtr.write_str("[a&&b]");
                }
                self.wtr.write_str("(?-u:[")?;
                for range in cls.iter() {
                    if range.start() == range.end() {
                        self.write_literal_class_byte(range.start())?;
                    } else if range.start() + 1 == range.end() {
                        self.write_literal_class_byte(range.start())?;
                        self.write_literal_class_byte(range.end())?;
                    } else {
                        self.write_literal_class_byte(range.start())?;
                        self.wtr.write_str("-")?;
                        self.write_literal_class_byte(range.end())?;
                    }
                }
                self.wtr.write_str("])")?;
            }
            HirKind::Look(ref look) => match *look {
                hir::Look::Start => {
                    self.wtr.write_str(r"\A")?;
                }
                hir::Look::End => {
                    self.wtr.write_str(r"\z")?;
                }
                hir::Look::StartLF => {
                    self.wtr.write_str("(?m:^)")?;
                }
                hir::Look::EndLF => {
                    self.wtr.write_str("(?m:$)")?;
                }
                hir::Look::StartCRLF => {
                    self.wtr.write_str("(?mR:^)")?;
                }
                hir::Look::EndCRLF => {
                    self.wtr.write_str("(?mR:$)")?;
                }
                hir::Look::WordAscii => {
                    self.wtr.write_str(r"(?-u:\b)")?;
                }
                hir::Look::WordAsciiNegate => {
                    self.wtr.write_str(r"(?-u:\B)")?;
                }
                hir::Look::WordUnicode => {
                    self.wtr.write_str(r"\b")?;
                }
                hir::Look::WordUnicodeNegate => {
                    self.wtr.write_str(r"\B")?;
                }
                hir::Look::WordStartAscii => {
                    self.wtr.write_str(r"(?-u:\b{start})")?;
                }
                hir::Look::WordEndAscii => {
                    self.wtr.write_str(r"(?-u:\b{end})")?;
                }
                hir::Look::WordStartUnicode => {
                    self.wtr.write_str(r"\b{start}")?;
                }
                hir::Look::WordEndUnicode => {
                    self.wtr.write_str(r"\b{end}")?;
                }
                hir::Look::WordStartHalfAscii => {
                    self.wtr.write_str(r"(?-u:\b{start-half})")?;
                }
                hir::Look::WordEndHalfAscii => {
                    self.wtr.write_str(r"(?-u:\b{end-half})")?;
                }
                hir::Look::WordStartHalfUnicode => {
                    self.wtr.write_str(r"\b{start-half}")?;
                }
                hir::Look::WordEndHalfUnicode => {
                    self.wtr.write_str(r"\b{end-half}")?;
                }
            },
            HirKind::Capture(hir::Capture { ref name, .. }) => {
                self.wtr.write_str("(")?;
                if let Some(ref name) = *name {
                    write!(self.wtr, "?P<{name}>")?;
                }
            }
            HirKind::Concat(_) | HirKind::Alternation(_) => {
                self.wtr.write_str(r"(?:")?;
            }
        }
        Ok(())
    }
    fn visit_post(&mut self, hir: &Hir) -> fmt::Result {
        match *hir.kind() {
            HirKind::Empty | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Look(_) => {}
            HirKind::Repetition(ref x) => {
                match (x.min, x.max) {
                    (0, Some(1)) => {
                        self.wtr.write_str("?")?;
                    }
                    (0, None) => {
                        self.wtr.write_str("*")?;
                    }
                    (1, None) => {
                        self.wtr.write_str("+")?;
                    }
                    (1, Some(1)) => {
                        return Ok(());
                    }
                    (m, None) => {
                        write!(self.wtr, "{{{m},}}")?;
                    }
                    (m, Some(n)) if m == n => {
                        write!(self.wtr, "{{{m}}}")?;
                        return Ok(());
                    }
                    (m, Some(n)) => {
                        write!(self.wtr, "{{{m},{n}}}")?;
                    }
                }
                if !x.greedy {
                    self.wtr.write_str("?")?;
                }
            }
            HirKind::Capture(_) | HirKind::Concat(_) | HirKind::Alternation(_) => {
                self.wtr.write_str(r")")?;
            }
        }
        Ok(())
    }
    fn visit_alternation_in(&mut self) -> fmt::Result {
        self.wtr.write_str("|")
    }
}
impl<W: fmt::Write> Writer<W> {
    fn write_literal_char(&mut self, c: char) -> fmt::Result {
        if is_meta_character(c) {
            self.wtr.write_str("\\")?;
        }
        self.wtr.write_char(c)
    }
    fn write_literal_byte(&mut self, b: u8) -> fmt::Result {
        if b <= 0x7F && !b.is_ascii_control() && !b.is_ascii_whitespace() {
            self.write_literal_char(char::try_from(b).unwrap())
        } else {
            write!(self.wtr, "(?-u:\\x{b:02X})")
        }
    }
    fn write_literal_class_byte(&mut self, b: u8) -> fmt::Result {
        if b <= 0x7F && !b.is_ascii_control() && !b.is_ascii_whitespace() {
            self.write_literal_char(char::try_from(b).unwrap())
        } else {
            write!(self.wtr, "\\x{b:02X}")
        }
    }
}

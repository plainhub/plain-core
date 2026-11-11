use crate::regex_engine::syntax::{ast, hir};
use std::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Parse(ast::Error),
    Translate(hir::Error),
}
impl From<ast::Error> for Error {
    fn from(err: ast::Error) -> Error {
        Error::Parse(err)
    }
}
impl From<hir::Error> for Error {
    fn from(err: hir::Error) -> Error {
        Error::Translate(err)
    }
}
impl std::error::Error for Error {}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Error::Parse(ref x) => x.fmt(f),
            Error::Translate(ref x) => x.fmt(f),
        }
    }
}
#[derive(Debug)]
pub struct Formatter<'e, E> {
    pattern: &'e str,
    err: &'e E,
    span: &'e ast::Span,
    aux_span: Option<&'e ast::Span>,
}
impl<'e> From<&'e ast::Error> for Formatter<'e, ast::ErrorKind> {
    fn from(err: &'e ast::Error) -> Self {
        Formatter {
            pattern: err.pattern(),
            err: err.kind(),
            span: err.span(),
            aux_span: err.auxiliary_span(),
        }
    }
}
impl<'e> From<&'e hir::Error> for Formatter<'e, hir::ErrorKind> {
    fn from(err: &'e hir::Error) -> Self {
        Formatter {
            pattern: err.pattern(),
            err: err.kind(),
            span: err.span(),
            aux_span: None,
        }
    }
}
impl<'e, E: core::fmt::Display> core::fmt::Display for Formatter<'e, E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let spans = Spans::from_formatter(self);
        if self.pattern.contains('\n') {
            let divider = repeat_char('~', 79);
            writeln!(f, "regex parse error:")?;
            writeln!(f, "{divider}")?;
            let notated = spans.notate();
            write!(f, "{notated}")?;
            writeln!(f, "{divider}")?;
            if !spans.multi_line.is_empty() {
                let mut notes = vec![];
                for span in &spans.multi_line {
                    notes.push(format!(
                        "on line {} (column {}) through line {} (column {})",
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column - 1
                    ));
                }
                writeln!(f, "{}", notes.join("\n"))?;
            }
            write!(f, "error: {}", self.err)?;
        } else {
            writeln!(f, "regex parse error:")?;
            let notated = Spans::from_formatter(self).notate();
            write!(f, "{notated}")?;
            write!(f, "error: {}", self.err)?;
        }
        Ok(())
    }
}
struct Spans<'p> {
    pattern: &'p str,
    line_number_width: usize,
    by_line: Vec<Vec<ast::Span>>,
    multi_line: Vec<ast::Span>,
}
impl<'p> Spans<'p> {
    fn from_formatter<'e, E: core::fmt::Display>(fmter: &'p Formatter<'e, E>) -> Spans<'p> {
        let mut line_count = fmter.pattern.lines().count();
        if fmter.pattern.ends_with('\n') {
            line_count += 1;
        }
        let line_number_width = if line_count <= 1 {
            0
        } else {
            line_count.to_string().len()
        };
        let mut spans = Spans {
            pattern: &fmter.pattern,
            line_number_width,
            by_line: vec![vec![]; line_count],
            multi_line: vec![],
        };
        spans.add(fmter.span.clone());
        if let Some(span) = fmter.aux_span {
            spans.add(span.clone());
        }
        spans
    }
    fn add(&mut self, span: ast::Span) {
        if span.is_one_line() {
            let i = span.start.line - 1;
            self.by_line[i].push(span);
            self.by_line[i].sort();
        } else {
            self.multi_line.push(span);
            self.multi_line.sort();
        }
    }
    fn notate(&self) -> String {
        let mut notated = String::new();
        for (i, line) in self.pattern.lines().enumerate() {
            if self.line_number_width > 0 {
                notated.push_str(&self.left_pad_line_number(i + 1));
                notated.push_str(": ");
            } else {
                notated.push_str("    ");
            }
            notated.push_str(line);
            notated.push('\n');
            if let Some(notes) = self.notate_line(i) {
                notated.push_str(&notes);
                notated.push('\n');
            }
        }
        notated
    }
    fn notate_line(&self, i: usize) -> Option<String> {
        let spans = &self.by_line[i];
        if spans.is_empty() {
            return None;
        }
        let mut notes = String::new();
        for _ in 0..self.line_number_padding() {
            notes.push(' ');
        }
        let mut pos = 0;
        for span in spans {
            for _ in pos..(span.start.column - 1) {
                notes.push(' ');
                pos += 1;
            }
            let note_len = span.end.column.saturating_sub(span.start.column);
            for _ in 0..core::cmp::max(1, note_len) {
                notes.push('^');
                pos += 1;
            }
        }
        Some(notes)
    }
    fn left_pad_line_number(&self, n: usize) -> String {
        let n = n.to_string();
        let pad = self.line_number_width.checked_sub(n.len()).unwrap();
        let mut result = repeat_char(' ', pad);
        result.push_str(&n);
        result
    }
    fn line_number_padding(&self) -> usize {
        if self.line_number_width == 0 {
            4
        } else {
            2 + self.line_number_width
        }
    }
}
fn repeat_char(c: char, count: usize) -> String {
    core::iter::repeat(c).take(count).collect()
}

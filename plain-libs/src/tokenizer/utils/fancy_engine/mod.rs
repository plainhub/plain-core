// Regex search adapted from fancy-regex 0.14.0 (SOURCE.json).
mod analyze;
mod compile;
mod error;
mod parse;
mod vm;

use regex_automata::{Input as RaInput, meta::Regex as RaRegex};
use std::fmt;

use analyze::analyze;
use compile::compile;
use error::{CompileError, Error, ParseError, Result};
use parse::Parser;
use vm::{OPTION_SKIPPED_EMPTY_MATCH, Prog};

const MAX_RECURSION: usize = 64;

pub struct SysRegex {
    inner: Engine,
    pattern: String,
}

enum Engine {
    Wrap { inner: RaRegex },
    Fancy { prog: Prog },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Match {
    start: usize,
    end: usize,
}
#[derive(Debug)]
struct Matches<'r, 't> {
    re: &'r SysRegex,
    text: &'t str,
    last_end: usize,
    last_match: Option<usize>,
}
impl<'r, 't> Iterator for Matches<'r, 't> {
    type Item = Result<Match>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.last_end > self.text.len() {
            return None;
        }

        let option_flags = if let Some(last_match) = self.last_match {
            if self.last_end > last_match {
                OPTION_SKIPPED_EMPTY_MATCH
            } else {
                0
            }
        } else {
            0
        };
        let mat =
            match self
                .re
                .find_from_pos_with_option_flags(self.text, self.last_end, option_flags)
            {
                Err(error) => return Some(Err(error)),
                Ok(None) => return None,
                Ok(Some(mat)) => mat,
            };

        if mat.start == mat.end {
            // This is an empty match. To ensure we make progress, start
            // the next search at the smallest possible starting position
            // of the next match following this one.
            self.last_end = next_utf8(self.text, mat.end);
            // Don't accept empty matches immediately following a match.
            // Just move on to the next match.
            if Some(mat.end) == self.last_match {
                return self.next();
            }
        } else {
            self.last_end = mat.end;
        }

        self.last_match = Some(mat.end);

        Some(Ok(mat))
    }
}

impl fmt::Debug for SysRegex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pattern)
    }
}
impl SysRegex {
    pub fn new(re: &str) -> super::super::Result<Self> {
        let raw_tree = Parser::parse(re)?;

        // Search from any position and capture the bounds of the entire match.
        let tree = Expr::Concat(vec![
            Expr::Repeat {
                child: Box::new(Expr::Any { newline: true }),
                lo: 0,
                hi: usize::MAX,
                greedy: false,
            },
            Expr::Group(Box::new(raw_tree)),
        ]);

        let info = analyze(&tree)?;

        let inner_info = &info.children[1].children[0];
        if !inner_info.hard {
            let mut re_cooked = String::new();
            inner_info.expr.to_str(&mut re_cooked, 0);
            let inner = compile::compile_inner(&re_cooked)?;
            return Ok(SysRegex {
                inner: Engine::Wrap { inner },
                pattern: re.to_owned(),
            });
        }

        let prog = compile(&info)?;
        Ok(SysRegex {
            inner: Engine::Fancy { prog },
            pattern: re.to_owned(),
        })
    }

    pub fn find_iter<'r, 't>(
        &'r self,
        text: &'t str,
    ) -> impl Iterator<Item = (usize, usize)> + use<'r, 't> {
        let mut matches = Matches {
            re: self,
            text,
            last_end: 0,
            last_match: None,
        };
        std::iter::from_fn(move || matches.next()?.ok().map(|m| (m.start, m.end)))
    }

    fn find_from_pos_with_option_flags(
        &self,
        text: &str,
        pos: usize,
        option_flags: u32,
    ) -> Result<Option<Match>> {
        match &self.inner {
            Engine::Wrap { inner, .. } => Ok(inner
                .search(&RaInput::new(text).span(pos..text.len()))
                .map(|m| Match {
                    start: m.start(),
                    end: m.end(),
                })),
            Engine::Fancy { prog } => {
                let result = vm::run(prog, text, pos, option_flags)?;
                Ok(result.map(|saves| Match {
                    start: saves[0],
                    end: saves[1],
                }))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Expr {
    Empty,

    Any {
        newline: bool,
    },

    Assertion(Assertion),

    Literal {
        val: String,

        casei: bool,
    },

    Concat(Vec<Expr>),

    Alt(Vec<Expr>),

    Group(Box<Expr>),

    LookAround(Box<Expr>, LookAround),

    Repeat {
        child: Box<Expr>,

        lo: usize,

        hi: usize,

        greedy: bool,
    },

    Delegate {
        inner: String,

        size: usize, // TODO: move into analysis result

        casei: bool,
    },

    Backref(usize),

    AtomicGroup(Box<Expr>),

    KeepOut,

    ContinueFromPreviousMatchEnd,

    BackrefExistsCondition(usize),

    Conditional {
        condition: Box<Expr>,

        true_branch: Box<Expr>,

        false_branch: Box<Expr>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LookAround {
    LookAhead,

    LookAheadNeg,

    LookBehind,

    LookBehindNeg,
}
fn push_usize(s: &mut String, x: usize) {
    if x >= 10 {
        push_usize(s, x / 10);
        s.push((b'0' + (x % 10) as u8) as char);
    } else {
        s.push((b'0' + (x as u8)) as char);
    }
}

fn is_special(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        | '#' => true,
        _ => false,
    }
}

fn push_quoted(buf: &mut String, s: &str) {
    for c in s.chars() {
        if is_special(c) {
            buf.push('\\');
        }
        buf.push(c);
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Assertion {
    StartText,

    EndText,

    StartLine { crlf: bool },

    EndLine { crlf: bool },

    LeftWordBoundary,

    RightWordBoundary,

    WordBoundary,

    NotWordBoundary,
}

impl Assertion {
    fn is_hard(&self) -> bool {
        use Assertion::*;
        matches!(
            self,
            // these will make regex-automata use PikeVM
            LeftWordBoundary | RightWordBoundary | WordBoundary | NotWordBoundary
        )
    }
}

impl Expr {
    fn to_str(&self, buf: &mut String, precedence: u8) {
        match *self {
            Expr::Empty => (),
            Expr::Any { newline } => buf.push_str(if newline { "(?s:.)" } else { "." }),
            Expr::Literal { ref val, casei } => {
                if casei {
                    buf.push_str("(?i:");
                }
                push_quoted(buf, val);
                if casei {
                    buf.push_str(")");
                }
            }
            Expr::Assertion(Assertion::StartText) => buf.push('^'),
            Expr::Assertion(Assertion::EndText) => buf.push('$'),
            Expr::Assertion(Assertion::StartLine { crlf: false }) => buf.push_str("(?m:^)"),
            Expr::Assertion(Assertion::EndLine { crlf: false }) => buf.push_str("(?m:$)"),
            Expr::Assertion(Assertion::StartLine { crlf: true }) => buf.push_str("(?Rm:^)"),
            Expr::Assertion(Assertion::EndLine { crlf: true }) => buf.push_str("(?Rm:$)"),
            Expr::Concat(ref children) => {
                if precedence > 1 {
                    buf.push_str("(?:");
                }
                for child in children {
                    child.to_str(buf, 2);
                }
                if precedence > 1 {
                    buf.push(')')
                }
            }
            Expr::Alt(ref children) => {
                if precedence > 0 {
                    buf.push_str("(?:");
                }
                for (i, child) in children.iter().enumerate() {
                    if i != 0 {
                        buf.push('|');
                    }
                    child.to_str(buf, 1);
                }
                if precedence > 0 {
                    buf.push(')');
                }
            }
            Expr::Group(ref child) => {
                buf.push('(');
                child.to_str(buf, 0);
                buf.push(')');
            }
            Expr::Repeat {
                ref child,
                lo,
                hi,
                greedy,
            } => {
                if precedence > 2 {
                    buf.push_str("(?:");
                }
                child.to_str(buf, 3);
                match (lo, hi) {
                    (0, 1) => buf.push('?'),
                    (0, usize::MAX) => buf.push('*'),
                    (1, usize::MAX) => buf.push('+'),
                    (lo, hi) => {
                        buf.push('{');
                        push_usize(buf, lo);
                        if lo != hi {
                            buf.push(',');
                            if hi != usize::MAX {
                                push_usize(buf, hi);
                            }
                        }
                        buf.push('}');
                    }
                }
                if !greedy {
                    buf.push('?');
                }
                if precedence > 2 {
                    buf.push(')');
                }
            }
            Expr::Delegate {
                ref inner, casei, ..
            } => {
                // at the moment, delegate nodes are just atoms
                if casei {
                    buf.push_str("(?i:");
                }
                buf.push_str(inner);
                if casei {
                    buf.push_str(")");
                }
            }
            _ => panic!("attempting to format hard expr"),
        }
    }
}

// precondition: ix > 0
fn prev_codepoint_ix(s: &str, mut ix: usize) -> usize {
    let bytes = s.as_bytes();
    loop {
        ix -= 1;
        // fancy bit magic for ranges 0..0x80 + 0xc0..
        if (bytes[ix] as i8) >= -0x40 {
            break;
        }
    }
    ix
}

fn codepoint_len(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}

fn next_utf8(text: &str, i: usize) -> usize {
    let b = match text.as_bytes().get(i) {
        None => return i + 1,
        Some(&b) => b,
    };
    i + codepoint_len(b)
}

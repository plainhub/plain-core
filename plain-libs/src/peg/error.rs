// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::peg::parser_state::{ParseAttempts, ParsingToken, RulesCallStack};
use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;
use core::fmt;
use core::mem;

use crate::peg::RuleType;
use crate::peg::position::Position;
use crate::peg::span::Span;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Error<R> {
    pub variant: ErrorVariant<R>,

    pub location: InputLocation,

    pub line_col: LineColLocation,
    inner: Box<ErrorInner<R>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ErrorInner<R> {
    path: Option<String>,
    line: String,
    continued_line: Option<String>,
    parse_attempts: Option<ParseAttempts<R>>,
}

impl<R: RuleType> core::error::Error for Error<R> {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ErrorVariant<R> {
    ParsingError {
        positives: Vec<R>,

        negatives: Vec<R>,
    },

    CustomError {
        message: String,
    },
}

impl<R: RuleType> core::error::Error for ErrorVariant<R> {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum InputLocation {
    Pos(usize),

    Span((usize, usize)),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LineColLocation {
    Pos((usize, usize)),

    Span((usize, usize), (usize, usize)),
}

impl From<Position<'_>> for LineColLocation {
    fn from(value: Position<'_>) -> Self {
        Self::Pos(value.line_col())
    }
}

impl From<Span<'_>> for LineColLocation {
    fn from(value: Span<'_>) -> Self {
        let (start, end) = value.split();
        Self::Span(start.line_col(), end.line_col())
    }
}

pub type RuleToMessageFn<R> = Box<dyn Fn(&R) -> Option<String>>;

pub type IsWhitespaceFn = Box<dyn Fn(String) -> bool>;

impl ParsingToken {
    pub fn is_whitespace(&self, is_whitespace: &IsWhitespaceFn) -> bool {
        match self {
            ParsingToken::Sensitive { token } => is_whitespace(token.clone()),
            ParsingToken::Insensitive { token } => is_whitespace(token.clone()),
            ParsingToken::Range { .. } => false,
            ParsingToken::BuiltInRule => false,
        }
    }
}

impl<R: RuleType> ParseAttempts<R> {
    fn tokens_helper_messages(
        &self,
        is_whitespace_fn: &IsWhitespaceFn,
        spacing: &str,
    ) -> Vec<String> {
        let mut helper_messages = Vec::new();
        let tokens_header_pairs = vec![
            (self.expected_tokens(), "expected"),
            (self.unexpected_tokens(), "unexpected"),
        ];

        for (tokens, header) in &tokens_header_pairs {
            if tokens.is_empty() {
                continue;
            }

            let mut helper_tokens_message = format!("{spacing}note: {header} ");
            helper_tokens_message.push_str(if tokens.len() == 1 {
                "token: "
            } else {
                "one of tokens: "
            });

            let expected_tokens_set: BTreeSet<String> = tokens
                .iter()
                .map(|token| {
                    if token.is_whitespace(is_whitespace_fn) {
                        String::from("WHITESPACE")
                    } else {
                        format!("`{token}`")
                    }
                })
                .collect();

            helper_tokens_message.push_str(
                &expected_tokens_set
                    .iter()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            helper_messages.push(helper_tokens_message);
        }

        helper_messages
    }
}

impl<R: RuleType> Error<R> {
    pub fn new_from_pos(variant: ErrorVariant<R>, pos: Position<'_>) -> Error<R> {
        let visualize_ws = pos.match_char('\n') || pos.match_char('\r');
        let line_of = pos.line_of();
        let line = if visualize_ws {
            visualize_whitespace(line_of)
        } else {
            line_of.replace(&['\r', '\n'][..], "")
        };
        Error {
            variant,
            location: InputLocation::Pos(pos.pos()),
            line_col: LineColLocation::Pos(pos.line_col()),
            inner: Box::new(ErrorInner {
                path: None,
                line,
                continued_line: None,
                parse_attempts: None,
            }),
        }
    }

    pub(crate) fn new_from_pos_with_parsing_attempts(
        variant: ErrorVariant<R>,
        pos: Position<'_>,
        parse_attempts: ParseAttempts<R>,
    ) -> Error<R> {
        let mut error = Self::new_from_pos(variant, pos);
        error.inner.parse_attempts = Some(parse_attempts);
        error
    }

    pub fn new_from_span(variant: ErrorVariant<R>, span: Span<'_>) -> Error<R> {
        let end = span.end_pos();
        let mut end_line_col = end.line_col();
        // end position is after a \n, so we want to point to the visual lf symbol
        if end_line_col.1 == 1 {
            let mut visual_end = end;
            visual_end.skip_back(1);
            let lc = visual_end.line_col();
            end_line_col = (lc.0, lc.1 + 1);
        };

        let mut line_iter = span.lines();
        let sl = line_iter.next().unwrap_or("");
        let mut chars = span.as_str().chars();
        let visualize_ws = matches!(chars.next(), Some('\n') | Some('\r'))
            || matches!(chars.last(), Some('\n') | Some('\r'));
        let start_line = if visualize_ws {
            visualize_whitespace(sl)
        } else {
            sl.to_owned().replace(&['\r', '\n'][..], "")
        };
        let ll = line_iter.last();
        let continued_line = if visualize_ws {
            ll.map(str::to_owned)
        } else {
            ll.map(visualize_whitespace)
        };

        Error {
            variant,
            location: InputLocation::Span((span.start(), end.pos())),
            line_col: LineColLocation::Span(span.start_pos().line_col(), end_line_col),
            inner: Box::new(ErrorInner {
                path: None,
                line: start_line,
                continued_line,
                parse_attempts: None,
            }),
        }
    }

    pub fn with_path(mut self, path: &str) -> Error<R> {
        self.inner.path = Some(path.to_owned());

        self
    }

    pub fn path(&self) -> Option<&str> {
        self.inner.path.as_deref()
    }

    pub fn line(&self) -> &str {
        self.inner.line.as_str()
    }

    pub fn renamed_rules<F>(mut self, f: F) -> Error<R>
    where
        F: FnMut(&R) -> String,
    {
        let variant = match self.variant {
            ErrorVariant::ParsingError {
                positives,
                negatives,
            } => {
                let message = Error::parsing_error_message(&positives, &negatives, f);
                ErrorVariant::CustomError { message }
            }
            variant => variant,
        };

        self.variant = variant;

        self
    }

    pub fn parse_attempts(&self) -> Option<ParseAttempts<R>> {
        self.inner.parse_attempts.clone()
    }

    pub fn parse_attempts_error(
        &self,
        input: &str,
        rule_to_message: &RuleToMessageFn<R>,
        is_whitespace: &IsWhitespaceFn,
    ) -> Option<Error<R>> {
        let attempts = if let Some(ref parse_attempts) = self.inner.parse_attempts {
            parse_attempts.clone()
        } else {
            return None;
        };

        let spacing = self.spacing() + "   ";
        let error_position = attempts.max_position;
        let message = {
            let mut help_lines: Vec<String> = Vec::new();
            help_lines.push(String::from("error: parsing error occurred."));

            // Note: at least one of `(un)expected_tokens` must not be empty.
            for tokens_helper_message in attempts.tokens_helper_messages(is_whitespace, &spacing) {
                help_lines.push(tokens_helper_message);
            }

            let call_stacks = attempts.call_stacks();
            // Group call stacks by their parents so that we can print common header and
            // several sub helper messages.
            let mut call_stacks_parents_groups: BTreeMap<Option<R>, Vec<RulesCallStack<R>>> =
                BTreeMap::new();
            for call_stack in call_stacks {
                call_stacks_parents_groups
                    .entry(call_stack.parent)
                    .or_default()
                    .push(call_stack);
            }

            for (group_parent, group) in call_stacks_parents_groups {
                if let Some(parent_rule) = group_parent {
                    let mut contains_meaningful_info = false;
                    help_lines.push(format!(
                        "{spacing}help: {}",
                        if let Some(message) = rule_to_message(&parent_rule) {
                            contains_meaningful_info = true;
                            message
                        } else {
                            String::from("[Unknown parent rule]")
                        }
                    ));
                    for call_stack in group {
                        if let Some(r) = call_stack.deepest.get_rule() {
                            if let Some(message) = rule_to_message(r) {
                                contains_meaningful_info = true;
                                help_lines.push(format!("{spacing}      - {message}"));
                            }
                        }
                    }
                    if !contains_meaningful_info {
                        // Have to remove useless line for unknown parent rule.
                        help_lines.pop();
                    }
                } else {
                    for call_stack in group {
                        // Note that `deepest` rule may be `None`. E.g. in case it corresponds
                        // to WHITESPACE expected token which has no parent rule (on the top level
                        // parsing).
                        if let Some(r) = call_stack.deepest.get_rule() {
                            let helper_message = rule_to_message(r);
                            if let Some(helper_message) = helper_message {
                                help_lines.push(format!("{spacing}help: {helper_message}"));
                            }
                        }
                    }
                }
            }

            help_lines.join("\n")
        };
        let error = Error::new_from_pos(
            ErrorVariant::CustomError { message },
            Position::new_internal(input, error_position),
        );
        Some(error)
    }

    fn start(&self) -> (usize, usize) {
        match self.line_col {
            LineColLocation::Pos(line_col) => line_col,
            LineColLocation::Span(start_line_col, _) => start_line_col,
        }
    }

    fn spacing(&self) -> String {
        let line = match self.line_col {
            LineColLocation::Pos((line, _)) => line,
            LineColLocation::Span((start_line, _), (end_line, _)) => cmp::max(start_line, end_line),
        };

        let line_str_len = format!("{line}").len();

        let mut spacing = String::new();
        for _ in 0..line_str_len {
            spacing.push(' ');
        }

        spacing
    }

    fn underline(&self) -> String {
        let mut underline = String::new();

        let mut start = self.start().1;
        let end = match self.line_col {
            LineColLocation::Span(_, (_, mut end)) => {
                let inverted_cols = start > end;
                if inverted_cols {
                    mem::swap(&mut start, &mut end);
                    start -= 1;
                    end += 1;
                }

                Some(end)
            }
            _ => None,
        };
        let offset = start - 1;
        let line_chars = self.inner.line.chars();

        for c in line_chars.take(offset) {
            match c {
                '\t' => underline.push('\t'),
                _ => underline.push(' '),
            }
        }

        if let Some(end) = end {
            underline.push('^');
            if end - start > 1 {
                for _ in 2..(end - start) {
                    underline.push('-');
                }
                underline.push('^');
            }
        } else {
            underline.push_str("^---")
        }

        underline
    }

    fn message(&self) -> String {
        self.variant.message().to_string()
    }

    fn parsing_error_message<F>(positives: &[R], negatives: &[R], mut f: F) -> String
    where
        F: FnMut(&R) -> String,
    {
        match (negatives.is_empty(), positives.is_empty()) {
            (false, false) => format!(
                "unexpected {}; expected {}",
                Error::enumerate(negatives, &mut f),
                Error::enumerate(positives, &mut f)
            ),
            (false, true) => format!("unexpected {}", Error::enumerate(negatives, &mut f)),
            (true, false) => format!("expected {}", Error::enumerate(positives, &mut f)),
            (true, true) => "unknown parsing error".to_owned(),
        }
    }

    fn enumerate<F>(rules: &[R], f: &mut F) -> String
    where
        F: FnMut(&R) -> String,
    {
        match rules.len() {
            1 => f(&rules[0]),
            2 => format!("{} or {}", f(&rules[0]), f(&rules[1])),
            l => {
                let non_separated = f(&rules[l - 1]);
                let separated = rules
                    .iter()
                    .take(l - 1)
                    .map(f)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{separated}, or {non_separated}")
            }
        }
    }

    pub(crate) fn format(&self) -> String {
        let spacing = self.spacing();
        let path = self
            .inner
            .path
            .as_ref()
            .map(|path| format!("{path}:"))
            .unwrap_or_default();

        let pair = (self.line_col.clone(), &self.inner.continued_line);
        if let (LineColLocation::Span(_, end), Some(continued_line)) = pair {
            let has_line_gap = end.0 - self.start().0 > 1;
            if has_line_gap {
                format!(
                    "{s    }--> {p}{ls}:{c}\n\
                     {s    } |\n\
                     {ls:w$} | {line}\n\
                     {s    } | ...\n\
                     {le:w$} | {continued_line}\n\
                     {s    } | {underline}\n\
                     {s    } |\n\
                     {s    } = {message}",
                    s = spacing,
                    w = spacing.len(),
                    p = path,
                    ls = self.start().0,
                    le = end.0,
                    c = self.start().1,
                    line = self.inner.line,
                    continued_line = continued_line,
                    underline = self.underline(),
                    message = self.message()
                )
            } else {
                format!(
                    "{s    }--> {p}{ls}:{c}\n\
                     {s    } |\n\
                     {ls:w$} | {line}\n\
                     {le:w$} | {continued_line}\n\
                     {s    } | {underline}\n\
                     {s    } |\n\
                     {s    } = {message}",
                    s = spacing,
                    w = spacing.len(),
                    p = path,
                    ls = self.start().0,
                    le = end.0,
                    c = self.start().1,
                    line = self.inner.line,
                    continued_line = continued_line,
                    underline = self.underline(),
                    message = self.message()
                )
            }
        } else {
            format!(
                "{s}--> {p}{l}:{c}\n\
                 {s} |\n\
                 {l} | {line}\n\
                 {s} | {underline}\n\
                 {s} |\n\
                 {s} = {message}",
                s = spacing,
                p = path,
                l = self.start().0,
                c = self.start().1,
                line = self.inner.line,
                underline = self.underline(),
                message = self.message()
            )
        }
    }
}

impl<R: RuleType> ErrorVariant<R> {
    pub fn message(&self) -> Cow<'_, str> {
        match self {
            ErrorVariant::ParsingError {
                positives,
                negatives,
            } => Cow::Owned(Error::parsing_error_message(positives, negatives, |r| {
                format!("{r:?}")
            })),
            ErrorVariant::CustomError { message } => Cow::Borrowed(message),
        }
    }
}

impl<R: RuleType> fmt::Display for Error<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl<R: RuleType> fmt::Display for ErrorVariant<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorVariant::ParsingError { .. } => write!(f, "parsing error: {}", self.message()),
            ErrorVariant::CustomError { .. } => write!(f, "{}", self.message()),
        }
    }
}

fn visualize_whitespace(input: &str) -> String {
    input.to_owned().replace('\r', "␍").replace('\n', "␊")
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/error/tests.rs"]
mod tests;

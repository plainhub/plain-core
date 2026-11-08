use crate::platform::stack_space::remaining_stack;
// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::{Debug, Display, Formatter};
use core::num::NonZeroUsize;
use core::ops::Deref; // used in BorrowedOrRc.as_str
use core::ops::Range;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::peg::RuleType;
use crate::peg::error::{Error, ErrorVariant};
use crate::peg::iterators::pairs::new;
use crate::peg::iterators::{QueueableToken, pairs};
use crate::peg::position::Position;
use crate::peg::span::Span;
use crate::peg::stack::Stack;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lookahead {
    Positive,

    Negative,

    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Atomicity {
    Atomic,

    CompoundAtomic,

    NonAtomic,
}

pub type ParseResult<S> = Result<S, S>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchDir {
    BottomToTop,

    TopToBottom,
}

static CALL_LIMIT: AtomicUsize = AtomicUsize::new(0);

pub fn set_call_limit(limit: Option<NonZeroUsize>) {
    CALL_LIMIT.store(
        limit.map(|limit| limit.get()).unwrap_or(0),
        Ordering::Relaxed,
    );
}

static ERROR_DETAIL: AtomicBool = AtomicBool::new(false);

pub fn set_error_detail(enabled: bool) {
    ERROR_DETAIL.store(enabled, Ordering::Relaxed);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LimitKind {
    Call,
    Stack,
}

const REQUIRED_STACK_SPACE: usize = 64 * 1024;

#[derive(Debug)]
struct CallLimitTracker {
    current_call_limit: Option<(usize, usize)>,
    limit_reached: Option<LimitKind>,
}

impl Default for CallLimitTracker {
    fn default() -> Self {
        let call_limit = CALL_LIMIT.load(Ordering::Relaxed);
        Self {
            current_call_limit: (call_limit > 0).then_some((0, call_limit)),
            limit_reached: None,
        }
    }
}

impl CallLimitTracker {
    fn limit_reached(&self) -> bool {
        self.limit_reached.is_some()
    }

    fn error_message(&self) -> Option<&'static str> {
        self.limit_reached.map(|cause| match cause {
            LimitKind::Call => "call limit reached",
            LimitKind::Stack => "stack limit reached",
        })
    }

    fn enter(&mut self, count_call: bool) -> bool {
        if self.limit_reached() {
            return false;
        }
        if count_call
            && self
                .current_call_limit
                .is_some_and(|(current, limit)| current >= limit)
        {
            self.limit_reached = Some(LimitKind::Call);
            return false;
        }
        let remaining = remaining_stack();
        self.enter_with_stack(count_call, remaining)
    }

    fn enter_with_stack(&mut self, count_call: bool, stack_space: Option<usize>) -> bool {
        if self.limit_reached() {
            return false;
        }
        let cause = if count_call
            && self
                .current_call_limit
                .is_some_and(|(current, limit)| current >= limit)
        {
            Some(LimitKind::Call)
        } else if stack_space.is_some_and(|remaining| remaining < REQUIRED_STACK_SPACE) {
            Some(LimitKind::Stack)
        } else {
            None
        };
        if let Some(cause) = cause {
            self.limit_reached = Some(cause);
            return false;
        }
        if count_call {
            if let Some((current, _)) = &mut self.current_call_limit {
                *current += 1;
            }
        }
        true
    }
}

const CALL_STACK_INITIAL_CAPACITY: usize = 20;

const EXPECTED_TOKENS_INITIAL_CAPACITY: usize = 30;

const CALL_STACK_CHILDREN_THRESHOLD: usize = 4;

#[derive(Debug, Hash, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum ParseAttempt<R> {
    Rule(R),

    Token,
}

impl<R> ParseAttempt<R> {
    pub fn get_rule(&self) -> Option<&R> {
        match self {
            ParseAttempt::Rule(r) => Some(r),
            ParseAttempt::Token => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RulesCallStack<R> {
    pub deepest: ParseAttempt<R>,

    pub parent: Option<R>,
}

impl<R> RulesCallStack<R> {
    fn new(deepest: ParseAttempt<R>) -> RulesCallStack<R> {
        RulesCallStack {
            deepest,
            parent: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParsingToken {
    Sensitive { token: String },
    Insensitive { token: String },
    Range { start: char, end: char },
    BuiltInRule,
}

impl Display for ParsingToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            ParsingToken::Sensitive { token } => write!(f, "{token}"),
            ParsingToken::Insensitive { token } => write!(f, "{}", token.to_uppercase()),
            ParsingToken::Range { start, end } => write!(f, "{start}..{end}"),
            ParsingToken::BuiltInRule => write!(f, "BUILTIN_RULE"),
        }
    }
}

#[derive(Debug, Clone)]
enum BorrowedOrArc<'i> {
    Borrowed(&'i str),
    Owned(Arc<String>),
}

impl<'i> BorrowedOrArc<'i> {
    fn as_str<'a: 'i>(&'a self) -> &'a str {
        match self {
            BorrowedOrArc::Borrowed(s) => s,
            BorrowedOrArc::Owned(s) => s.deref(),
        }
    }
}

impl From<Cow<'static, str>> for BorrowedOrArc<'_> {
    fn from(value: Cow<'static, str>) -> Self {
        match value {
            Cow::Borrowed(s) => Self::Borrowed(s),
            Cow::Owned(s) => Self::Owned(Arc::new(s)),
        }
    }
}

#[derive(Debug, Clone)]
enum SpanOrLiteral<'i> {
    Span(Span<'i>),
    Literal(BorrowedOrArc<'i>),
}

impl<'i> SpanOrLiteral<'i> {
    #[inline]
    fn as_borrowed_or_rc(&self) -> BorrowedOrArc<'i> {
        match self {
            Self::Span(s) => BorrowedOrArc::Borrowed(s.as_str()),
            Self::Literal(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParseAttempts<R> {
    enabled: bool,

    pub call_stacks: Vec<RulesCallStack<R>>,

    expected_tokens: Vec<ParsingToken>,

    unexpected_tokens: Vec<ParsingToken>,

    pub max_position: usize,
}

impl<R: RuleType> ParseAttempts<R> {
    pub fn new() -> Self {
        Self {
            call_stacks: Vec::with_capacity(CALL_STACK_INITIAL_CAPACITY),
            expected_tokens: Vec::with_capacity(EXPECTED_TOKENS_INITIAL_CAPACITY),
            unexpected_tokens: Vec::with_capacity(EXPECTED_TOKENS_INITIAL_CAPACITY),
            max_position: 0,
            enabled: ERROR_DETAIL.load(Ordering::Relaxed),
        }
    }

    fn call_stacks_number(&self) -> usize {
        self.call_stacks.len()
    }

    pub fn expected_tokens(&self) -> Vec<ParsingToken> {
        self.expected_tokens
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn unexpected_tokens(&self) -> Vec<ParsingToken> {
        self.unexpected_tokens
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn call_stacks(&self) -> Vec<RulesCallStack<R>> {
        self.call_stacks
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn try_add_new_stack_rule(&mut self, rule: R, start_index: usize) {
        let mut non_token_call_stacks = Vec::new();
        let mut token_call_stack_met = false;
        for call_stack in self.call_stacks.iter().skip(start_index) {
            if matches!(call_stack.deepest, ParseAttempt::Token) {
                token_call_stack_met = true;
            } else {
                non_token_call_stacks.push(call_stack.clone())
            }
        }
        if token_call_stack_met && non_token_call_stacks.is_empty() {
            // If `non_token_call_stacks` is not empty we wouldn't like to add a new standalone
            // `RulesCallStack::new(ParseAttempt::Token)` (that will later be transformed into a
            // rule) as soon as it doesn't give us any useful additional info.
            non_token_call_stacks.push(RulesCallStack::new(ParseAttempt::Token));
        }
        self.call_stacks
            .splice(start_index.., non_token_call_stacks);

        let children_number_over_threshold =
            self.call_stacks_number() - start_index >= CALL_STACK_CHILDREN_THRESHOLD;
        if children_number_over_threshold {
            self.call_stacks.truncate(start_index);
            self.call_stacks
                .push(RulesCallStack::new(ParseAttempt::Rule(rule)));
        } else {
            for call_stack in self.call_stacks.iter_mut().skip(start_index) {
                if matches!(call_stack.deepest, ParseAttempt::Token) {
                    call_stack.deepest = ParseAttempt::Rule(rule);
                } else {
                    call_stack.parent = Some(rule);
                }
            }
        }
    }

    fn try_add_new_token(
        &mut self,
        token: ParsingToken,
        start_position: usize,
        position: usize,
        negative_lookahead: bool,
    ) {
        let target_vec_push_token = |attempts: &mut ParseAttempts<R>| {
            let target_vec = if negative_lookahead {
                &mut attempts.unexpected_tokens
            } else {
                &mut attempts.expected_tokens
            };
            target_vec.push(token);
        };

        if position > self.max_position {
            if negative_lookahead && start_position > self.max_position {
                // We encountered a sequence under negative lookahead.
                // We would like to track only first failed token in this sequence (which
                // `start_position` should be equal to `self.max_position`).
                return;
            }
            target_vec_push_token(self);

            if negative_lookahead {
                // In case of successful parsing of token under negative lookahead the only
                // thing we'd like to do is to track the token in the `unexpected_tokens`.
                return;
            }
            self.max_position = position;
            self.expected_tokens.clear();
            self.unexpected_tokens.clear();
            self.call_stacks.clear();
            self.call_stacks
                .push(RulesCallStack::new(ParseAttempt::Token));
        } else if position == self.max_position {
            target_vec_push_token(self);
            self.call_stacks
                .push(RulesCallStack::new(ParseAttempt::Token));
        }
    }

    fn nullify_expected_tokens(&mut self, new_max_position: usize) {
        self.call_stacks.clear();
        self.expected_tokens.clear();
        self.unexpected_tokens.clear();
        self.max_position = new_max_position;
    }
}

impl<R: RuleType> Default for ParseAttempts<R> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ParserState<'i, R: RuleType> {
    position: Position<'i>,

    queue: Vec<QueueableToken<'i, R>>,

    lookahead: Lookahead,

    pos_attempts: Vec<R>,

    neg_attempts: Vec<R>,

    attempt_pos: usize,

    atomicity: Atomicity,

    stack: Stack<SpanOrLiteral<'i>>,

    call_tracker: CallLimitTracker,

    parse_attempts: ParseAttempts<R>,
}

pub fn state<'i, R: RuleType, F>(input: &'i str, f: F) -> Result<pairs::Pairs<'i, R>, Error<R>>
where
    F: FnOnce(Box<ParserState<'i, R>>) -> ParseResult<Box<ParserState<'i, R>>>,
{
    let state = ParserState::new(input);

    match f(state) {
        Ok(state) if !state.call_tracker.limit_reached() => {
            let len = state.queue.len();
            Ok(new(Rc::new(state.queue), input, None, 0, len))
        }
        Ok(mut state) | Err(mut state) => {
            let variant = if let Some(message) = state.call_tracker.error_message() {
                ErrorVariant::CustomError {
                    message: message.to_owned(),
                }
            } else {
                state.pos_attempts.sort();
                state.pos_attempts.dedup();
                state.neg_attempts.sort();
                state.neg_attempts.dedup();
                ErrorVariant::ParsingError {
                    positives: state.pos_attempts.clone(),
                    negatives: state.neg_attempts.clone(),
                }
            };

            if state.parse_attempts.enabled {
                Err(Error::new_from_pos_with_parsing_attempts(
                    variant,
                    Position::new_internal(input, state.attempt_pos),
                    state.parse_attempts.clone(),
                ))
            } else {
                Err(Error::new_from_pos(
                    variant,
                    Position::new_internal(input, state.attempt_pos),
                ))
            }
        }
    }
}

impl<'i, R: RuleType> ParserState<'i, R> {
    pub fn new(input: &'i str) -> Box<Self> {
        Box::new(ParserState {
            position: Position::from_start(input),
            queue: vec![],
            lookahead: Lookahead::None,
            pos_attempts: vec![],
            neg_attempts: vec![],
            attempt_pos: 0,
            atomicity: Atomicity::NonAtomic,
            stack: Stack::new(),
            call_tracker: Default::default(),
            parse_attempts: ParseAttempts::new(),
        })
    }

    pub fn get_parse_attempts(&self) -> &ParseAttempts<R> {
        &self.parse_attempts
    }

    pub fn position(&self) -> &Position<'i> {
        &self.position
    }

    pub fn atomicity(&self) -> Atomicity {
        self.atomicity
    }

    #[inline]
    fn inc_call_check_limit(mut self: Box<Self>) -> ParseResult<Box<Self>> {
        if !self.call_tracker.enter(true) {
            return Err(self);
        }
        Ok(self)
    }

    #[inline]
    pub fn check_stack_limit(mut self: Box<Self>) -> ParseResult<Box<Self>> {
        if !self.call_tracker.enter(false) {
            return Err(self);
        }
        Ok(self)
    }

    #[inline]
    pub fn rule<F>(mut self: Box<Self>, rule: R, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        // Position from which this `rule` starts parsing.
        let actual_pos = self.position.pos();
        // Remember index of the `self.queue` element that will be associated with this `rule`.
        let index = self.queue.len();

        let (pos_attempts_index, neg_attempts_index) = if actual_pos == self.attempt_pos {
            (self.pos_attempts.len(), self.neg_attempts.len())
        } else {
            // Attempts have not been cleared yet since the attempt_pos is older.
            (0, 0)
        };

        if self.lookahead == Lookahead::None && self.atomicity != Atomicity::Atomic {
            // Pair's position will only be known after running the closure.
            self.queue.push(QueueableToken::Start {
                end_token_index: 0,
                input_pos: actual_pos,
            });
        }

        // Remember attempts number before `f` call.
        // In `track` using this variable we can say, how many attempts were added
        // during children rules traversal.
        let attempts = self.attempts_at(actual_pos);
        // Number of call stacks present in `self.parse_attempts` before `f` call.
        // We need to remember this number only in case there wasn't found any farther attempt.
        // E.g. we are handling rule, on start position of which may be tested two
        // children rules. At the moment we'll return from `f` call below,
        // there will be two more children rules in `self.parse_attempts` that we'll
        // consider to be the children of current `rule`.
        let mut remember_call_stacks_number = self.parse_attempts.call_stacks_number();
        // Max parsing attempt position at the moment of `rule` handling.
        // It case it's raised during children rules handling, it means
        // we've made a parsing progress.
        let remember_max_position = self.parse_attempts.max_position;

        let result = f(self);

        let mut try_add_rule_to_stack = |new_state: &mut Box<ParserState<'_, R>>| {
            if new_state.parse_attempts.max_position > remember_max_position {
                // It means that one of `match_string` or e.g. `match_insensitive` function calls
                // have already erased `self.parse_attempts.call_stacks` and that previously
                // remembered values are not valid anymore.
                remember_call_stacks_number = 0;
            }
            if !matches!(new_state.atomicity, Atomicity::Atomic) {
                new_state
                    .parse_attempts
                    .try_add_new_stack_rule(rule, remember_call_stacks_number);
            }
        };

        match result {
            Ok(mut new_state) => {
                if new_state.lookahead == Lookahead::Negative {
                    new_state.track(
                        rule,
                        actual_pos,
                        pos_attempts_index,
                        neg_attempts_index,
                        attempts,
                    );
                }

                if new_state.lookahead == Lookahead::None
                    && new_state.atomicity != Atomicity::Atomic
                {
                    // Index of `QueueableToken::End` token added below
                    // that corresponds to previously added `QueueableToken::Start` token.
                    let new_index = new_state.queue.len();
                    match new_state.queue[index] {
                        QueueableToken::Start {
                            ref mut end_token_index,
                            ..
                        } => *end_token_index = new_index,
                        _ => unreachable!(),
                    };

                    let new_pos = new_state.position.pos();

                    new_state.queue.push(QueueableToken::End {
                        start_token_index: index,
                        rule,
                        tag: None,
                        input_pos: new_pos,
                    });
                }

                // Note, that we need to count positive parsing results too, because we can fail in
                // optional rule call inside which may lie the farthest
                // parsed token.
                if new_state.parse_attempts.enabled {
                    try_add_rule_to_stack(&mut new_state);
                }
                Ok(new_state)
            }
            Err(mut new_state) => {
                if new_state.lookahead != Lookahead::Negative {
                    new_state.track(
                        rule,
                        actual_pos,
                        pos_attempts_index,
                        neg_attempts_index,
                        attempts,
                    );
                    if new_state.parse_attempts.enabled {
                        try_add_rule_to_stack(&mut new_state);
                    }
                }

                if new_state.lookahead == Lookahead::None
                    && new_state.atomicity != Atomicity::Atomic
                {
                    new_state.queue.truncate(index);
                }

                Err(new_state)
            }
        }
    }

    #[inline]
    pub fn tag_node(mut self: Box<Self>, tag: &'i str) -> ParseResult<Box<Self>> {
        if self.lookahead != Lookahead::None {
            return Ok(self);
        }
        if let Some(QueueableToken::End { tag: old, .. }) = self.queue.last_mut() {
            *old = Some(tag)
        }
        Ok(self)
    }

    fn attempts_at(&self, pos: usize) -> usize {
        if self.attempt_pos == pos {
            self.pos_attempts.len() + self.neg_attempts.len()
        } else {
            0
        }
    }

    fn track(
        &mut self,
        rule: R,
        pos: usize,
        pos_attempts_index: usize,
        neg_attempts_index: usize,
        prev_attempts: usize,
    ) {
        if self.atomicity == Atomicity::Atomic {
            return;
        }

        // If nested rules made no progress, there is no use to report them; it's only useful to
        // track the current rule, the exception being when only one attempt has been made during
        // the children rules.
        let curr_attempts = self.attempts_at(pos);
        if curr_attempts > prev_attempts && curr_attempts - prev_attempts == 1 {
            return;
        }

        if pos == self.attempt_pos {
            self.pos_attempts.truncate(pos_attempts_index);
            self.neg_attempts.truncate(neg_attempts_index);
        }

        if pos > self.attempt_pos {
            self.pos_attempts.clear();
            self.neg_attempts.clear();
            self.attempt_pos = pos;
        }

        let attempts = if self.lookahead != Lookahead::Negative {
            &mut self.pos_attempts
        } else {
            &mut self.neg_attempts
        };

        if pos == self.attempt_pos {
            attempts.push(rule);
        }
    }

    #[inline]
    pub fn sequence<F>(mut self: Box<Self>, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        let token_index = self.queue.len();
        let initial_pos = self.position;

        let result = f(self.checkpoint());

        match result {
            Ok(new_state) => Ok(new_state.checkpoint_ok()),
            Err(mut new_state) => {
                // Restore the initial position and truncate the token queue.
                new_state.position = initial_pos;
                new_state.queue.truncate(token_index);
                Err(new_state.restore())
            }
        }
    }

    #[inline]
    pub fn repeat<F>(mut self: Box<Self>, mut f: F) -> ParseResult<Box<Self>>
    where
        F: FnMut(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        let mut result = f(self);

        loop {
            match result {
                Ok(state) => result = f(state),
                Err(state) if state.call_tracker.limit_reached() => return Err(state),
                Err(state) => return Ok(state),
            };
        }
    }

    #[inline]
    pub fn optional<F>(mut self: Box<Self>, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        match f(self) {
            Err(state) if state.call_tracker.limit_reached() => Err(state),
            Ok(state) | Err(state) => Ok(state),
        }
    }

    fn handle_token_parse_result(
        &mut self,
        start_position: usize,
        token: ParsingToken,
        parse_succeeded: bool,
    ) {
        // New position after tracked parsed element for case of `parse_succeeded` is true.
        // Position of parsing failure otherwise.
        let current_pos = self.position.pos();

        if parse_succeeded {
            if self.lookahead == Lookahead::Negative {
                self.parse_attempts
                    .try_add_new_token(token, start_position, current_pos, true);
            } else if current_pos > self.parse_attempts.max_position {
                self.parse_attempts.nullify_expected_tokens(current_pos);
            }
        } else if self.lookahead != Lookahead::Negative {
            self.parse_attempts
                .try_add_new_token(token, start_position, current_pos, false);
        }
    }

    #[inline]
    pub fn match_char_by<F>(mut self: Box<Self>, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(char) -> bool,
    {
        let start_position = self.position.pos();
        let succeeded = self.position.match_char_by(f);
        if self.parse_attempts.enabled {
            let token = ParsingToken::BuiltInRule;
            self.handle_token_parse_result(start_position, token, succeeded);
        }
        if succeeded { Ok(self) } else { Err(self) }
    }

    #[inline]
    pub fn match_string(mut self: Box<Self>, string: &str) -> ParseResult<Box<Self>> {
        let start_position = self.position.pos();
        let succeeded = self.position.match_string(string);
        if self.parse_attempts.enabled {
            let token = ParsingToken::Sensitive {
                token: String::from(string),
            };
            self.handle_token_parse_result(start_position, token, succeeded);
        }
        if succeeded { Ok(self) } else { Err(self) }
    }

    #[inline]
    pub fn stack_push_literal(
        mut self: Box<Self>,
        string: impl Into<Cow<'static, str>>,
    ) -> ParseResult<Box<Self>> {
        // convert string into a Literal, and then into a BorrowedOrRc
        self.stack
            .push(SpanOrLiteral::Literal(string.into().into()));
        Ok(self)
    }

    #[inline]
    pub fn match_insensitive(mut self: Box<Self>, string: &str) -> ParseResult<Box<Self>> {
        let start_position: usize = self.position().pos();
        let succeeded = self.position.match_insensitive(string);
        if self.parse_attempts.enabled {
            let token = ParsingToken::Insensitive {
                token: String::from(string),
            };
            self.handle_token_parse_result(start_position, token, succeeded);
        }
        if succeeded { Ok(self) } else { Err(self) }
    }

    #[inline]
    pub fn match_range(mut self: Box<Self>, range: Range<char>) -> ParseResult<Box<Self>> {
        let start_position = self.position().pos();
        let token = ParsingToken::Range {
            start: range.start,
            end: range.end,
        };
        let succeeded = self.position.match_range(range);
        if self.parse_attempts.enabled {
            self.handle_token_parse_result(start_position, token, succeeded);
        }
        if succeeded { Ok(self) } else { Err(self) }
    }

    #[inline]
    pub fn skip(mut self: Box<Self>, n: usize) -> ParseResult<Box<Self>> {
        if self.position.skip(n) {
            Ok(self)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn skip_until(mut self: Box<Self>, strings: &[&str]) -> ParseResult<Box<Self>> {
        self.position.skip_until(strings);
        Ok(self)
    }

    #[inline]
    pub fn start_of_input(self: Box<Self>) -> ParseResult<Box<Self>> {
        if self.position.at_start() {
            Ok(self)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn end_of_input(self: Box<Self>) -> ParseResult<Box<Self>> {
        if self.position.at_end() {
            Ok(self)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn lookahead<F>(mut self: Box<Self>, is_positive: bool, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        let initial_lookahead = self.lookahead;

        self.lookahead = if is_positive {
            match initial_lookahead {
                Lookahead::None | Lookahead::Positive => Lookahead::Positive,
                Lookahead::Negative => Lookahead::Negative,
            }
        } else {
            match initial_lookahead {
                Lookahead::None | Lookahead::Positive => Lookahead::Negative,
                Lookahead::Negative => Lookahead::Positive,
            }
        };

        let initial_pos = self.position;

        let result = f(self.checkpoint());

        let result_state = match result {
            Ok(mut new_state) => {
                new_state.position = initial_pos;
                new_state.lookahead = initial_lookahead;
                Ok(new_state.restore())
            }
            Err(mut new_state) => {
                new_state.position = initial_pos;
                new_state.lookahead = initial_lookahead;
                Err(new_state.restore())
            }
        };

        let exhausted = match &result_state {
            Ok(state) | Err(state) => state.call_tracker.limit_reached(),
        };
        if exhausted {
            return Err(result_state.unwrap_or_else(|state| state));
        }
        if is_positive {
            result_state
        } else {
            match result_state {
                Ok(state) => Err(state),
                Err(state) => Ok(state),
            }
        }
    }

    #[inline]
    pub fn atomic<F>(mut self: Box<Self>, atomicity: Atomicity, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        // In case child parsing call is another `atomic` it will have its own atomicity status.
        let initial_atomicity = self.atomicity;
        // In case child atomicity is the same as we've demanded, we shouldn't do nothing.
        // E.g. we have the following rules:
        // * RootRule = @{ InnerRule }
        // * InnerRule = @{ ... }
        let should_toggle = self.atomicity != atomicity;

        // Note that we take atomicity of the top rule and not of the leaf (inner).
        if should_toggle {
            self.atomicity = atomicity;
        }

        let result = f(self);

        match result {
            Ok(mut new_state) => {
                if should_toggle {
                    new_state.atomicity = initial_atomicity;
                }
                Ok(new_state)
            }
            Err(mut new_state) => {
                if should_toggle {
                    new_state.atomicity = initial_atomicity;
                }
                Err(new_state)
            }
        }
    }

    #[inline]
    pub fn stack_push<F>(mut self: Box<Self>, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.inc_call_check_limit()?;
        let start = self.position;

        let result = f(self);

        match result {
            Ok(mut state) => {
                let end = state.position;
                state.stack.push(SpanOrLiteral::Span(start.span(&end)));
                Ok(state)
            }
            Err(state) => Err(state),
        }
    }

    #[inline]
    pub fn stack_peek(self: Box<Self>) -> ParseResult<Box<Self>> {
        let string = self
            .stack
            .peek()
            .expect("peek was called on empty stack")
            .as_borrowed_or_rc();
        self.match_string(string.as_str())
    }

    #[inline]
    pub fn stack_pop(mut self: Box<Self>) -> ParseResult<Box<Self>> {
        let string = self
            .stack
            .pop()
            .expect("pop was called on empty stack")
            .as_borrowed_or_rc();
        self.match_string(string.as_str())
    }

    #[inline]
    pub fn stack_match_peek_slice(
        mut self: Box<Self>,
        start: i32,
        end: Option<i32>,
        match_dir: MatchDir,
    ) -> ParseResult<Box<Self>> {
        let range = match constrain_idxs(start, end, self.stack.len()) {
            Some(r) => r,
            None => return Err(self),
        };
        // return true if an empty sequence is requested
        if range.end <= range.start {
            return Ok(self);
        }

        let mut position = self.position;
        let result = {
            let mut iter_b2t = self.stack[range].iter();
            let matcher =
                |span: &SpanOrLiteral<'_>| position.match_string(span.as_borrowed_or_rc().as_str());
            match match_dir {
                MatchDir::BottomToTop => iter_b2t.all(matcher),
                MatchDir::TopToBottom => iter_b2t.rev().all(matcher),
            }
        };
        if result {
            self.position = position;
            Ok(self)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn stack_match_peek(self: Box<Self>) -> ParseResult<Box<Self>> {
        self.stack_match_peek_slice(0, None, MatchDir::TopToBottom)
    }

    #[inline]
    pub fn stack_match_pop(mut self: Box<Self>) -> ParseResult<Box<Self>> {
        let mut position = self.position;
        let mut result = true;
        while let Some(span) = self.stack.pop() {
            result = position.match_string(span.as_borrowed_or_rc().as_str());
            if !result {
                break;
            }
        }

        if result {
            self.position = position;
            Ok(self)
        } else {
            Err(self)
        }
    }

    #[inline]
    pub fn stack_drop(mut self: Box<Self>) -> ParseResult<Box<Self>> {
        match self.stack.pop() {
            Some(_) => Ok(self),
            None => Err(self),
        }
    }

    #[inline]
    pub fn restore_on_err<F>(mut self: Box<Self>, f: F) -> ParseResult<Box<Self>>
    where
        F: FnOnce(Box<Self>) -> ParseResult<Box<Self>>,
    {
        self = self.check_stack_limit()?;
        match f(self.checkpoint()) {
            Ok(state) => Ok(state.checkpoint_ok()),
            Err(state) => Err(state.restore()),
        }
    }

    // Mark the current state as a checkpoint and return the `Box`.
    #[inline]
    pub(crate) fn checkpoint(mut self: Box<Self>) -> Box<Self> {
        self.stack.snapshot();
        self
    }

    // The checkpoint was cleared successfully
    // so remove it without touching other stack state.
    #[inline]
    pub(crate) fn checkpoint_ok(mut self: Box<Self>) -> Box<Self> {
        self.stack.clear_snapshot();
        self
    }

    // Restore the current state to the most recent checkpoint.
    #[inline]
    pub(crate) fn restore(mut self: Box<Self>) -> Box<Self> {
        self.stack.restore();
        self
    }
}

fn constrain_idxs(start: i32, end: Option<i32>, len: usize) -> Option<Range<usize>> {
    let start_norm = normalize_index(start, len)?;
    let end_norm = end.map_or(Some(len), |e| normalize_index(e, len))?;
    Some(start_norm..end_norm)
}

fn normalize_index(i: i32, len: usize) -> Option<usize> {
    if i > len as i32 {
        None
    } else if i >= 0 {
        Some(i as usize)
    } else {
        let real_i = len as i32 + i;
        if real_i >= 0 {
            Some(real_i as usize)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/parser_state/test.rs"]
mod test;

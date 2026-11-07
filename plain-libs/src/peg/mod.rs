#![forbid(unsafe_code)]
// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

pub extern crate alloc;

pub use crate::peg::parser::Parser;
pub use crate::peg::parser_state::{
    Atomicity, Lookahead, MatchDir, ParseResult, ParserState, set_call_limit, set_error_detail,
    state,
};
pub use crate::peg::position::Position;
pub use crate::peg::span::{Lines, LinesSpan, Span, merge_spans};
pub use crate::peg::stack::Stack;
pub use crate::peg::token::Token;
use core::fmt::Debug;
use core::hash::Hash;

pub mod error;
pub mod iterators;
mod macros;
mod parser;
mod parser_state;
mod position;
mod span;
mod stack;
mod token;

pub trait RuleType: Copy + Debug + Eq + Hash + Ord {}

impl<T: Copy + Debug + Eq + Hash + Ord> RuleType for T {}

pub use crate::{consumes_to, fails_with, parses_to};

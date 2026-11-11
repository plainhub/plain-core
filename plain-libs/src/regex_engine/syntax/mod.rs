#![forbid(unsafe_code)]
pub use crate::regex_engine::syntax::{
    error::Error,
    parser::{Parser, ParserBuilder, parse},
    unicode::UnicodeWordError,
};
use std::string::String;
pub mod ast;
mod debug;
mod either;
mod error;
pub mod hir;
mod parser;
mod rank;
mod unicode;
mod unicode_tables;
pub mod utf8;
pub fn escape(text: &str) -> String {
    let mut quoted = String::new();
    escape_into(text, &mut quoted);
    quoted
}
pub fn escape_into(text: &str, buf: &mut String) {
    buf.reserve(text.len());
    for c in text.chars() {
        if is_meta_character(c) {
            buf.push('\\');
        }
        buf.push(c);
    }
}
pub fn is_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        | '#' | '&' | '-' | '~' => true,
        _ => false,
    }
}
pub fn is_escapeable_character(c: char) -> bool {
    if is_meta_character(c) {
        return true;
    }
    if !c.is_ascii() {
        return false;
    }
    match c {
        '0'..='9' | 'A'..='Z' | 'a'..='z' => false,
        '<' | '>' => false,
        _ => true,
    }
}
pub fn is_word_character(c: char) -> bool {
    try_is_word_character(c).expect("unicode-perl feature must be enabled")
}
pub fn try_is_word_character(c: char) -> core::result::Result<bool, UnicodeWordError> {
    unicode::is_word_character(c)
}
pub fn is_word_byte(c: u8) -> bool {
    match c {
        b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => true,
        _ => false,
    }
}

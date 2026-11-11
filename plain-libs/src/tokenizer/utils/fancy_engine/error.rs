use core::fmt;
use crate::regex_engine::automata::meta::BuildError as RaBuildError;

pub type Result<T> = ::core::result::Result<T, Error>;

pub type ParseErrorPosition = usize;

#[derive(Clone, Debug)]
pub enum Error {
    ParseError(ParseErrorPosition, ParseError),

    CompileError(CompileError),

    RuntimeError(RuntimeError),
}

#[derive(Clone, Debug)]
pub enum ParseError {
    GeneralParseError(String),

    UnclosedOpenParen,

    InvalidRepeat,

    RecursionExceeded,

    TrailingBackslash,

    InvalidEscape(String),

    UnclosedUnicodeName,

    InvalidHex,

    InvalidCodepointValue,

    InvalidClass,

    UnknownFlag(String),

    NonUnicodeUnsupported,

    InvalidBackref,

    TargetNotRepeatable,

    InvalidGroupName,

    InvalidGroupNameBackref(String),
}

#[derive(Clone, Debug)]
pub enum CompileError {
    MatchSizeOverflow,
    InnerError(RaBuildError),

    LookBehindNotConst,

    InvalidBackref,

    NamedBackrefOnly,
}

#[derive(Clone, Debug)]
pub enum RuntimeError {
    StackOverflow,

    BacktrackLimitExceeded,
}

impl ::std::error::Error for Error {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::GeneralParseError(s) => write!(f, "General parsing error: {}", s),
            ParseError::UnclosedOpenParen => {
                write!(f, "Opening parenthesis without closing parenthesis")
            }
            ParseError::InvalidRepeat => write!(f, "Invalid repeat syntax"),
            ParseError::RecursionExceeded => write!(f, "Pattern too deeply nested"),
            ParseError::TrailingBackslash => write!(f, "Backslash without following character"),
            ParseError::InvalidEscape(s) => write!(f, "Invalid escape: {}", s),
            ParseError::UnclosedUnicodeName => write!(f, "Unicode escape not closed"),
            ParseError::InvalidHex => write!(f, "Invalid hex escape"),
            ParseError::InvalidCodepointValue => {
                write!(f, "Invalid codepoint for hex or unicode escape")
            }
            ParseError::InvalidClass => write!(f, "Invalid character class"),
            ParseError::UnknownFlag(s) => write!(f, "Unknown group flag: {}", s),
            ParseError::NonUnicodeUnsupported => write!(f, "Disabling Unicode not supported"),
            ParseError::InvalidBackref => write!(f, "Invalid back reference"),
            ParseError::InvalidGroupName => write!(f, "Could not parse group name"),
            ParseError::InvalidGroupNameBackref(s) => {
                write!(f, "Invalid group name in back reference: {}", s)
            }
            ParseError::TargetNotRepeatable => write!(f, "Target of repeat operator is invalid"),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompileError::MatchSizeOverflow => write!(f, "Minimum match size is too large"),
            CompileError::InnerError(e) => write!(f, "Regex error: {}", e),
            CompileError::LookBehindNotConst => {
                write!(f, "Look-behind assertion without constant size")
            }
            CompileError::InvalidBackref => write!(f, "Invalid back reference"),
            CompileError::NamedBackrefOnly => write!(
                f,
                "Numbered backref/call not allowed because named group was used, use a named backref instead"
            ),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeError::StackOverflow => write!(f, "Max stack size exceeded for backtracking"),
            RuntimeError::BacktrackLimitExceeded => {
                write!(f, "Max limit for backtracking count exceeded")
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ParseError(position, parse_error) => {
                write!(f, "Parsing error at position {}: {}", position, parse_error)
            }
            Error::CompileError(compile_error) => {
                write!(f, "Error compiling regex: {}", compile_error)
            }
            Error::RuntimeError(runtime_error) => {
                write!(f, "Error executing regex: {}", runtime_error)
            }
        }
    }
}

impl From<CompileError> for Error {
    fn from(compile_error: CompileError) -> Self {
        Error::CompileError(compile_error)
    }
}

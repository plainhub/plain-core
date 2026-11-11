use crate::xml::errors::{Error, IllFormedError, SyntaxError};
use crate::xml::events::{BytesRef, Event};
use crate::xml::parser::{DtdParser, ElementParser, Parser, PiParser};
use crate::xml::reader::state::ReaderState;
use std::io;
use std::ops::Range;
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Config {
    pub allow_dangling_amp: bool,
    pub allow_unmatched_ends: bool,
    pub check_comments: bool,
    pub check_end_names: bool,
    pub expand_empty_elements: bool,
    pub trim_markup_names_in_closing_tags: bool,
    pub trim_text_start: bool,
    pub trim_text_end: bool,
}
impl Config {
    #[inline]
    pub fn trim_text(&mut self, trim: bool) {
        self.trim_text_start = trim;
        self.trim_text_end = trim;
    }
    #[inline]
    pub fn enable_all_checks(&mut self, enable: bool) {
        self.check_comments = enable;
        self.check_end_names = enable;
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            allow_dangling_amp: false,
            allow_unmatched_ends: false,
            check_comments: false,
            check_end_names: true,
            expand_empty_elements: false,
            trim_markup_names_in_closing_tags: true,
            trim_text_start: false,
            trim_text_end: false,
        }
    }
}
macro_rules! read_event_impl {
    ($ self : ident , $ buf : ident , $ reader : expr , $ read_until_close : ident ) => {{
        let event = loop {
            break match $self.state.state {
                ParseState::Init => {
                    $reader.remove_utf8_bom()?;
                    $self.state.state = ParseState::InsideText;
                    continue;
                }
                ParseState::InsideRef => {
                    let start = $self.state.offset;
                    match $reader.read_ref($buf, &mut $self.state.offset) {
                        ReadRefResult::Ref(bytes) => {
                            $self.state.state = ParseState::InsideText;
                            Ok(Event::GeneralRef(BytesRef::wrap(
                                &bytes[1..bytes.len() - 1],
                            )))
                        }
                        ReadRefResult::UpToEof(bytes) if $self.state.config.allow_dangling_amp => {
                            $self.state.state = ParseState::Done;
                            Ok(Event::Text($self.state.emit_text(bytes)?))
                        }
                        ReadRefResult::UpToEof(_) => {
                            $self.state.state = ParseState::Done;
                            $self.state.last_error_offset = start;
                            Err(Error::IllFormed(IllFormedError::UnclosedReference))
                        }
                        ReadRefResult::UpToRef(bytes) if $self.state.config.allow_dangling_amp => {
                            Ok(Event::Text($self.state.emit_text(bytes)?))
                        }
                        ReadRefResult::UpToRef(_) => {
                            $self.state.last_error_offset = start;
                            Err(Error::IllFormed(IllFormedError::UnclosedReference))
                        }
                        ReadRefResult::UpToMarkup(bytes)
                            if $self.state.config.allow_dangling_amp =>
                        {
                            $self.state.state = ParseState::InsideMarkup;
                            Ok(Event::Text($self.state.emit_text(bytes)?))
                        }
                        ReadRefResult::UpToMarkup(_) => {
                            $self.state.state = ParseState::InsideMarkup;
                            $self.state.last_error_offset = start;
                            Err(Error::IllFormed(IllFormedError::UnclosedReference))
                        }
                        ReadRefResult::Err(e) => Err(e),
                    }
                }
                ParseState::InsideText => {
                    if $self.state.config.trim_text_start {
                        $reader.skip_whitespace(&mut $self.state.offset)?;
                    }
                    match $reader.read_text($buf, &mut $self.state.offset) {
                        ReadTextResult::Markup(buf) => {
                            $self.state.state = ParseState::InsideMarkup;
                            $buf = buf;
                            continue;
                        }
                        ReadTextResult::Ref(buf) => {
                            $self.state.state = ParseState::InsideRef;
                            $buf = buf;
                            continue;
                        }
                        ReadTextResult::UpToMarkup(bytes) => {
                            $self.state.state = ParseState::InsideMarkup;
                            Ok(Event::Text($self.state.emit_text(bytes)?))
                        }
                        ReadTextResult::UpToRef(bytes) => {
                            $self.state.state = ParseState::InsideRef;
                            Ok(Event::Text($self.state.emit_text(bytes)?))
                        }
                        ReadTextResult::UpToEof(bytes) => {
                            $self.state.state = ParseState::Done;
                            let event = $self.state.emit_text(bytes)?;
                            if event.is_empty() {
                                Ok(Event::Eof)
                            } else {
                                Ok(Event::Text(event))
                            }
                        }
                        ReadTextResult::Err(e) => Err(e),
                    }
                }
                ParseState::InsideMarkup => $self.$read_until_close($buf),
                ParseState::InsideEmpty => Ok(Event::End($self.state.close_expanded_empty())),
                ParseState::Done => Ok(Event::Eof),
            };
        };
        match event {
            Err(Error::IllFormed(_)) => {}
            Err(_) | Ok(Event::Eof) => $self.state.state = ParseState::Done,
            _ => {}
        }
        event
    }};
}
macro_rules! read_until_close {
    ($ self : ident , $ buf : ident , $ reader : expr ) => {{
        $self.state.state = ParseState::InsideText;
        let start = $self.state.offset;
        match $reader.peek_one() {
            Ok(Some(b'!')) => match $reader.read_bang_element($buf, &mut $self.state.offset) {
                Ok((bang_type, bytes)) => $self.state.emit_bang(bang_type, bytes),
                Err(e) => {
                    $self.state.last_error_offset = start;
                    Err(e)
                }
            },
            Ok(Some(b'/')) => {
                match $reader.read_with(ElementParser::Outside, $buf, &mut $self.state.offset) {
                    Ok(bytes) => $self.state.emit_end(bytes),
                    Err(e) => {
                        $self.state.last_error_offset = start;
                        Err(e)
                    }
                }
            }
            Ok(Some(b'?')) => {
                match $reader.read_with(PiParser(false), $buf, &mut $self.state.offset) {
                    Ok(bytes) => $self.state.emit_question_mark(bytes),
                    Err(e) => {
                        $self.state.last_error_offset = start;
                        Err(e)
                    }
                }
            }
            Ok(Some(_)) => {
                match $reader.read_with(ElementParser::Outside, $buf, &mut $self.state.offset) {
                    Ok(bytes) => $self.state.emit_start(bytes),
                    Err(e) => {
                        $self.state.last_error_offset = start;
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                $self.state.last_error_offset = start;
                Err(Error::Syntax(SyntaxError::UnclosedTag))
            }
            Err(e) => Err(Error::from(e)),
        }
    }};
}
macro_rules ! read_to_end { ($ self : expr , $ end : expr , $ buf : expr , $ read_event : ident , $ clear : block ) => { { let config = $ self . config_mut () ; let trim = config . trim_text_start ; config . trim_text_start = false ; let start = $ self . buffer_position () ; let mut depth = 0 ; loop { $ clear let end = $ self . buffer_position () ; match $ self .$ read_event ($ buf)  { Err (e) => { $ self . config_mut () . trim_text_start = trim ; return Err (e) ; } Ok (Event :: Start (e)) if e . name () == $ end => depth += 1 , Ok (Event :: End (e)) if e . name () == $ end => { if depth == 0 { $ self . config_mut () . trim_text_start = trim ; break start .. end ; } depth -= 1 ; } Ok (Event :: Eof) => { $ self . config_mut () . trim_text_start = trim ; return Err (Error :: missed_end ($ end)) ; } _ => () , } } } } ; }
mod buffered_reader;
mod ns_reader;
mod slice_reader;
mod state;
pub use ns_reader::NsReader;
pub type Span = Range<u64>;
#[derive(Clone, Debug)]
enum ParseState {
    Init,
    InsideRef,
    InsideMarkup,
    InsideText,
    InsideEmpty,
    Done,
}
#[derive(Debug, Clone)]
pub struct Reader<R> {
    reader: R,
    state: ReaderState,
}
impl<R> Reader<R> {
    pub fn from_reader(reader: R) -> Self {
        Self {
            reader,
            state: ReaderState::default(),
        }
    }
    pub const fn config(&self) -> &Config {
        &self.state.config
    }
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.state.config
    }
}
impl<R> Reader<R> {
    pub fn into_inner(self) -> R {
        self.reader
    }
    pub const fn get_ref(&self) -> &R {
        &self.reader
    }
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }
    pub const fn buffer_position(&self) -> u64 {
        self.state.offset
    }
    pub const fn error_position(&self) -> u64 {
        self.state.last_error_offset
    }
}
impl<R> Reader<R> {
    fn read_event_impl<'i, B>(&mut self, mut buf: B) -> Result<Event<'i>, Error>
    where
        R: XmlSource<'i, B>,
    {
        read_event_impl!(self, buf, self.reader, read_until_close)
    }
    fn read_until_close<'i, B>(&mut self, buf: B) -> Result<Event<'i>, Error>
    where
        R: XmlSource<'i, B>,
    {
        read_until_close!(self, buf, self.reader)
    }
}
#[derive(Debug)]
enum ReadTextResult<'r, B> {
    Markup(B),
    Ref(B),
    UpToMarkup(&'r str),
    UpToRef(&'r str),
    UpToEof(&'r str),
    Err(Error),
}
#[derive(Debug)]
enum ReadRefResult<'r> {
    Ref(&'r str),
    UpToEof(&'r str),
    UpToRef(&'r str),
    UpToMarkup(&'r str),
    Err(Error),
}
trait XmlSource<'r, B> {
    fn remove_utf8_bom(&mut self) -> io::Result<()>;
    fn read_text(&mut self, buf: B, position: &mut u64) -> ReadTextResult<'r, B>;
    fn read_ref(&mut self, buf: B, position: &mut u64) -> ReadRefResult<'r>;
    fn read_with<P>(&mut self, parser: P, buf: B, position: &mut u64) -> Result<&'r str, Error>
    where
        P: Parser;
    fn read_bang_element(
        &mut self,
        buf: B,
        position: &mut u64,
    ) -> Result<(BangType, &'r str), Error>;
    fn skip_whitespace(&mut self, position: &mut u64) -> io::Result<()>;
    fn peek_one(&mut self) -> io::Result<Option<u8>>;
}
#[derive(Debug, PartialEq)]
enum BangType {
    CData,
    Comment,
    DocType(DtdParser),
}
impl BangType {
    #[inline(always)]
    const fn new(byte: Option<u8>) -> Result<Self, SyntaxError> {
        Ok(match byte {
            Some(b'[') => Self::CData,
            Some(b'-') => Self::Comment,
            Some(b'D') | Some(b'd') => Self::DocType(DtdParser::BeforeInternalSubset(0)),
            _ => return Err(SyntaxError::InvalidBangMarkup),
        })
    }
    #[inline(always)]
    fn feed(&mut self, buf: &[u8], chunk: &[u8]) -> Option<usize> {
        match self {
            Self::Comment => {
                for i in crate::memchr::memchr_iter(b'>', chunk) {
                    if buf.len() + i > 5 {
                        if chunk[..i].ends_with(b"--") {
                            return Some(i);
                        }
                        if i == 1 && buf.ends_with(b"-") && chunk[0] == b'-' {
                            return Some(i);
                        }
                        if i == 0 && buf.ends_with(b"--") {
                            return Some(i);
                        }
                    }
                }
            }
            Self::CData => {
                for i in crate::memchr::memchr_iter(b'>', chunk) {
                    if chunk[..i].ends_with(b"]]") {
                        return Some(i);
                    }
                    if i == 1 && buf.ends_with(b"]") && chunk[0] == b']' {
                        return Some(i);
                    }
                    if i == 0 && buf.ends_with(b"]]") {
                        return Some(i);
                    }
                }
            }
            Self::DocType(parser) => return parser.feed(buf, chunk),
        }
        None
    }
    #[inline]
    const fn to_err(&self) -> SyntaxError {
        match self {
            Self::CData => SyntaxError::UnclosedCData,
            Self::Comment => SyntaxError::UnclosedComment,
            Self::DocType(_) => SyntaxError::UnclosedDoctype,
        }
    }
}

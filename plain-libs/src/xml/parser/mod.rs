use crate::xml::errors::SyntaxError;
mod comment;
mod dtd;
mod element;
mod pi;
pub use comment::CommentParser;
pub(crate) use dtd::DtdParser;
pub use element::ElementParser;
pub use pi::PiParser;
pub trait Parser {
    fn feed(&mut self, bytes: &[u8]) -> Option<usize>;
    fn eof_error(self, content: &[u8]) -> SyntaxError;
}

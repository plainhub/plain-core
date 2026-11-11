pub(crate) use self::conn::Conn;
pub(crate) use self::decode::Decoder;
pub(crate) use self::dispatch::Dispatcher;
pub(crate) use self::encode::{EncodedBuf, Encoder};
pub(crate) use self::io::MINIMUM_MAX_BUFFER_SIZE;
use crate::http::{HeaderMap, Method};
use crate::httparse::ParserConfig;
use crate::hyper::body::DecodedLength;
use crate::hyper::proto::{BodyLength, MessageHead};
use bytes::BytesMut;
mod conn;
mod decode;
pub(crate) mod dispatch;
mod encode;
mod io;
mod role;
cfg_client! { pub (crate) type ClientTransaction = role::Client ; }
cfg_server! { pub (crate) type ServerTransaction = role::Server ; }
pub(crate) trait Http1Transaction {
    type Incoming;
    type Outgoing: Default;
    fn parse(bytes: &mut BytesMut, ctx: ParseContext<'_>) -> ParseResult<Self::Incoming>;
    fn encode(enc: Encode<'_, Self::Outgoing>, dst: &mut Vec<u8>) -> crate::hyper::Result<Encoder>;
    fn on_error(err: &crate::hyper::Error) -> Option<MessageHead<Self::Outgoing>>;
    fn is_client() -> bool {
        !Self::is_server()
    }
    fn is_server() -> bool {
        !Self::is_client()
    }
    fn should_error_on_parse_eof() -> bool {
        Self::is_client()
    }
    fn should_read_first() -> bool {
        Self::is_server()
    }
    fn update_date() {}
}
pub(crate) type ParseResult<T> = Result<Option<ParsedMessage<T>>, crate::hyper::error::Parse>;
#[derive(Debug)]
pub(crate) struct ParsedMessage<T> {
    head: MessageHead<T>,
    decode: DecodedLength,
    expect_continue: bool,
    keep_alive: bool,
    wants_upgrade: bool,
}
pub(crate) struct ParseContext<'a> {
    cached_headers: &'a mut Option<HeaderMap>,
    req_method: &'a mut Option<Method>,
    h1_parser_config: ParserConfig,
    h1_max_headers: Option<usize>,
    preserve_header_case: bool,
    h09_responses: bool,
    on_informational: &'a mut Option<crate::hyper::ext::OnInformational>,
}
pub(crate) struct Encode<'a, T> {
    head: &'a mut MessageHead<T>,
    body: Option<BodyLength>,
    keep_alive: bool,
    req_method: &'a mut Option<Method>,
    title_case_headers: bool,
    date_header: bool,
}
#[derive(Clone, Copy, Debug)]
struct Wants(u8);
impl Wants {
    const EMPTY: Wants = Wants(0b00);
    const EXPECT: Wants = Wants(0b01);
    const UPGRADE: Wants = Wants(0b10);
    #[must_use]
    fn add(self, other: Wants) -> Wants {
        Wants(self.0 | other.0)
    }
    fn contains(&self, other: Wants) -> bool {
        (self.0 & other.0) == other.0
    }
}

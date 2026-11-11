use super::machine::TryParse;
use crate::http::header::{HeaderMap, HeaderName, HeaderValue};
use crate::httparse::Status;
use crate::tungstenite::error::Result;
pub const MAX_HEADERS: usize = 124;
pub(crate) trait FromHttparse<T>: Sized {
    fn from_httparse(raw: T) -> Result<Self>;
}
impl<'b: 'h, 'h> FromHttparse<&'b [crate::httparse::Header<'h>]> for HeaderMap {
    fn from_httparse(raw: &'b [crate::httparse::Header<'h>]) -> Result<Self> {
        let mut headers = HeaderMap::new();
        for h in raw {
            headers.append(
                HeaderName::from_bytes(h.name.as_bytes())?,
                HeaderValue::from_bytes(h.value)?,
            );
        }
        Ok(headers)
    }
}
impl TryParse for HeaderMap {
    fn try_parse(buf: &[u8]) -> Result<Option<(usize, Self)>> {
        let mut hbuffer = [crate::httparse::EMPTY_HEADER; MAX_HEADERS];
        Ok(match crate::httparse::parse_headers(buf, &mut hbuffer)? {
            Status::Partial => None,
            Status::Complete((size, hdr)) => Some((size, HeaderMap::from_httparse(hdr)?)),
        })
    }
}

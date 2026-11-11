use crate::http::header::Entry;
use crate::http::header::ValueIter;
use crate::http::header::{self, HeaderMap, HeaderName, HeaderValue};
use crate::http::{Method, StatusCode, Version};
use crate::hyper::body::DecodedLength;
use crate::hyper::common::date;
use crate::hyper::error::Parse;
use crate::hyper::ext::HeaderCaseMap;
use crate::hyper::headers;
use crate::hyper::proto::RequestHead;
use crate::hyper::proto::h1::{
    Encode, Encoder, Http1Transaction, ParseContext, ParseResult, ParsedMessage,
};
use crate::hyper::proto::{BodyLength, MessageHead, RequestLine};
use bytes::Bytes;
use bytes::BytesMut;
use smallvec::{SmallVec, smallvec, smallvec_inline};
use std::fmt::{self, Write as _};
use std::mem::MaybeUninit;
pub(crate) const DEFAULT_MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30;
const MAX_URI_LEN: usize = (u16::MAX - 1) as usize;
macro_rules! header_name {
    ($bytes : expr) => {{
        {
            match HeaderName::from_bytes($bytes) {
                Ok(name) => name,
                Err(e) => maybe_panic!(e),
            }
        }
    }};
}
macro_rules! header_value {
    ($bytes : expr) => {{ { unsafe { HeaderValue::from_maybe_shared_unchecked($bytes) } } }};
}
macro_rules! maybe_panic { ($($arg : tt) *) => ({ let _err = ($($arg) *) ; if cfg ! (debug_assertions) { panic ! ("{:?}" , _err) ; } else { error ! ("Internal Hyper error, please report {:?}" , _err) ; return Err (Parse::Internal) } }) }
pub(super) fn parse_headers<T>(
    bytes: &mut BytesMut,
    prev_len: Option<usize>,
    ctx: ParseContext<'_>,
) -> ParseResult<T::Incoming>
where
    T: Http1Transaction,
{
    if bytes.is_empty() {
        return Ok(None);
    }
    let _entered = trace_span!("parse_headers");
    if let Some(prev_len) = prev_len {
        if !is_complete_fast(bytes, prev_len) {
            return Ok(None);
        }
    }
    T::parse(bytes, ctx)
}
fn is_complete_fast(bytes: &[u8], prev_len: usize) -> bool {
    let start = prev_len.saturating_sub(3);
    let bytes = &bytes[start..];
    for (i, b) in bytes.iter().copied().enumerate() {
        if b == b'\r' {
            if bytes[i + 1..].chunks(3).next() == Some(&b"\n\r\n"[..]) {
                return true;
            }
        } else if b == b'\n'
            && (bytes.get(i + 1) == Some(&b'\n')
                || bytes[i + 1..].chunks(2).next() == Some(&b"\r\n"[..]))
        {
            return true;
        }
    }
    false
}
pub(super) fn encode_headers<T>(
    enc: Encode<'_, T::Outgoing>,
    dst: &mut Vec<u8>,
) -> crate::hyper::Result<Encoder>
where
    T: Http1Transaction,
{
    let _entered = trace_span!("encode_headers");
    T::encode(enc, dst)
}
pub(crate) enum Client {}
pub(crate) enum Server {}
impl Http1Transaction for Server {
    type Incoming = RequestLine;
    type Outgoing = StatusCode;
    fn parse(buf: &mut BytesMut, ctx: ParseContext<'_>) -> ParseResult<RequestLine> {
        debug_assert!(!buf.is_empty(), "parse called with empty buf");
        let mut keep_alive;
        let is_http_11;
        let subject;
        let version;
        let len;
        let headers_len;
        let method;
        let path_range;
        let mut headers_indices: SmallVec<[MaybeUninit<HeaderIndices>; DEFAULT_MAX_HEADERS]> =
            match ctx.h1_max_headers {
                Some(cap) => smallvec ! [MaybeUninit::uninit () ; cap],
                None => smallvec_inline ! [MaybeUninit::uninit () ; DEFAULT_MAX_HEADERS],
            };
        {
            let mut headers: SmallVec<
                [MaybeUninit<crate::httparse::Header<'_>>; DEFAULT_MAX_HEADERS],
            > = match ctx.h1_max_headers {
                Some(cap) => smallvec ! [MaybeUninit::uninit () ; cap],
                None => smallvec_inline ! [MaybeUninit::uninit () ; DEFAULT_MAX_HEADERS],
            };
            trace!(bytes = buf.len(), "Request.parse");
            let mut req = crate::httparse::Request::new(&mut []);
            let bytes = buf.as_ref();
            match ctx.h1_parser_config.parse_request_with_uninit_headers(
                &mut req,
                bytes,
                &mut headers,
            ) {
                Ok(crate::httparse::Status::Complete(parsed_len)) => {
                    trace!("Request.parse Complete({})", parsed_len);
                    len = parsed_len;
                    let uri = req.path.expect("httparse completed");
                    if uri.len() > MAX_URI_LEN {
                        return Err(Parse::UriTooLong);
                    }
                    method =
                        Method::from_bytes(req.method.expect("httparse completed").as_bytes())?;
                    path_range = Server::record_path_range(bytes, uri);
                    version = if req.version.expect("httparse completed") == 1 {
                        keep_alive = true;
                        is_http_11 = true;
                        Version::HTTP_11
                    } else {
                        keep_alive = false;
                        is_http_11 = false;
                        Version::HTTP_10
                    };
                    record_header_indices(bytes, req.headers, &mut headers_indices)?;
                    headers_len = req.headers.len();
                }
                Ok(crate::httparse::Status::Partial) => return Ok(None),
                Err(crate::httparse::Error::Token) => {
                    return Err({
                        if req.method.is_none() {
                            Parse::Method
                        } else {
                            debug_assert!(req.path.is_none());
                            Parse::Uri
                        }
                    });
                }
                Err(err) => return Err(err.into()),
            }
        };
        let slice = buf.split_to(len).freeze();
        let uri = {
            let uri_bytes = slice.slice_ref(&slice[path_range]);
            crate::http::Uri::from_maybe_shared(uri_bytes)?
        };
        subject = RequestLine(method, uri);
        let mut decoder = DecodedLength::ZERO;
        let mut expect_continue = false;
        let mut con_len = None;
        let mut is_cl = false;
        let mut is_te = false;
        let mut is_te_chunked = false;
        let mut wants_upgrade = subject.0 == Method::CONNECT;
        let mut header_case_map = if ctx.preserve_header_case {
            Some(HeaderCaseMap::default())
        } else {
            None
        };
        let mut headers = ctx.cached_headers.take().unwrap_or_default();
        headers.reserve(headers_len);
        for header in &headers_indices[..headers_len] {
            let header = unsafe { header.assume_init_ref() };
            let name = header_name!(&slice[header.name.0..header.name.1]);
            let value = header_value!(slice.slice(header.value.0..header.value.1));
            match name {
                header::TRANSFER_ENCODING => {
                    if !is_http_11 {
                        debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                        return Err(Parse::transfer_encoding_unexpected());
                    }
                    is_te = true;
                    if is_cl && con_len.take().is_some() {
                        headers.remove(header::CONTENT_LENGTH);
                    }
                    if headers::is_chunked_(&value) {
                        is_te_chunked = true;
                        decoder = DecodedLength::CHUNKED;
                    } else {
                        is_te_chunked = false;
                    }
                }
                header::CONTENT_LENGTH => {
                    is_cl = true;
                    if is_te {
                        continue;
                    }
                    let len = headers::content_length_parse(&value)
                        .ok_or_else(Parse::content_length_invalid)?;
                    if let Some(prev) = con_len {
                        if prev != len {
                            debug!(
                                "multiple Content-Length headers with different values: [{}, {}]",
                                prev, len,
                            );
                            return Err(Parse::content_length_invalid());
                        }
                        continue;
                    }
                    decoder = DecodedLength::checked_new(len)?;
                    con_len = Some(len);
                }
                header::CONNECTION => {
                    if keep_alive {
                        keep_alive = !headers::connection_close(&value);
                    } else {
                        keep_alive = headers::connection_keep_alive(&value);
                    }
                }
                header::EXPECT => {
                    expect_continue = value.as_bytes().eq_ignore_ascii_case(b"100-continue");
                }
                header::UPGRADE => {
                    wants_upgrade = is_http_11;
                }
                _ => (),
            }
            if let Some(header_case_map) = &mut header_case_map {
                header_case_map.append(&name, slice.slice(header.name.0..header.name.1));
            }
            headers.append(name, value);
        }
        if is_te && !is_te_chunked {
            debug!("request with transfer-encoding header, but not chunked, bad request");
            return Err(Parse::transfer_encoding_invalid());
        }
        if is_te && is_cl {
            keep_alive = false;
        }
        let mut extensions = crate::http::Extensions::default();
        if let Some(header_case_map) = header_case_map {
            extensions.insert(header_case_map);
        }
        *ctx.req_method = Some(subject.0.clone());
        Ok(Some(ParsedMessage {
            head: MessageHead {
                version,
                subject,
                headers,
                extensions,
            },
            decode: decoder,
            expect_continue,
            keep_alive,
            wants_upgrade,
        }))
    }
    fn encode(
        mut msg: Encode<'_, Self::Outgoing>,
        dst: &mut Vec<u8>,
    ) -> crate::hyper::Result<Encoder> {
        trace!(
            "Server::encode status={:?}, body={:?}, req_method={:?}",
            msg.head.subject, msg.body, msg.req_method
        );
        let mut wrote_len = false;
        let (ret, is_last) = if msg.head.subject == StatusCode::SWITCHING_PROTOCOLS {
            (Ok(()), true)
        } else if msg.req_method == &Some(Method::CONNECT) && msg.head.subject.is_success() {
            wrote_len = true;
            (Ok(()), true)
        } else if msg.head.subject.is_informational() {
            warn!("response with 1xx status code not supported");
            *msg.head = MessageHead::default();
            msg.head.subject = StatusCode::INTERNAL_SERVER_ERROR;
            msg.body = None;
            (
                Err(crate::hyper::Error::new_user_unsupported_status_code()),
                true,
            )
        } else {
            (Ok(()), !msg.keep_alive)
        };
        let orig_len = dst.len();
        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        let custom_reason_phrase = msg.head.extensions.get::<crate::hyper::ext::ReasonPhrase>();
        if msg.head.version == Version::HTTP_11
            && msg.head.subject == StatusCode::OK
            && custom_reason_phrase.is_none()
        {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
        } else {
            match msg.head.version {
                Version::HTTP_10 => extend(dst, b"HTTP/1.0 "),
                Version::HTTP_11 => extend(dst, b"HTTP/1.1 "),
                Version::HTTP_2 => {
                    debug!("response with HTTP2 version coerced to HTTP/1.1");
                    extend(dst, b"HTTP/1.1 ");
                }
                other => panic!("unexpected response version: {other:?}"),
            }
            extend(dst, msg.head.subject.as_str().as_bytes());
            extend(dst, b" ");
            if let Some(reason) = custom_reason_phrase {
                extend(dst, reason.as_bytes());
            } else {
                extend(
                    dst,
                    msg.head
                        .subject
                        .canonical_reason()
                        .unwrap_or("<none>")
                        .as_bytes(),
                );
            }
            extend(dst, b"\r\n");
        }
        let orig_headers;
        let extensions = std::mem::take(&mut msg.head.extensions);
        let orig_headers = match extensions.get::<HeaderCaseMap>() {
            None if msg.title_case_headers => {
                orig_headers = HeaderCaseMap::default();
                Some(&orig_headers)
            }
            orig_headers => orig_headers,
        };
        let encoder = if let Some(orig_headers) = orig_headers {
            Self::encode_headers_with_original_case(
                msg,
                dst,
                is_last,
                orig_len,
                wrote_len,
                orig_headers,
            )?
        } else {
            Self::encode_headers_with_lower_case(msg, dst, is_last, orig_len, wrote_len)?
        };
        ret.map(|()| encoder)
    }
    fn on_error(err: &crate::hyper::Error) -> Option<MessageHead<Self::Outgoing>> {
        use crate::hyper::error::Kind;
        let status = match *err.kind() {
            Kind::Parse(Parse::Method)
            | Kind::Parse(Parse::Header(_))
            | Kind::Parse(Parse::Uri)
            | Kind::Parse(Parse::Version) => StatusCode::BAD_REQUEST,
            Kind::Parse(Parse::TooLarge) => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Kind::Parse(Parse::UriTooLong) => StatusCode::URI_TOO_LONG,
            _ => return None,
        };
        debug!("sending automatic response ({}) for parse error", status);
        let msg = MessageHead {
            subject: status,
            ..Default::default()
        };
        Some(msg)
    }
    fn is_server() -> bool {
        true
    }
    fn update_date() {
        date::update();
    }
}
impl Server {
    fn can_have_body(method: Option<&Method>, status: StatusCode) -> bool {
        Server::can_chunked(method, status)
    }
    fn can_chunked(method: Option<&Method>, status: StatusCode) -> bool {
        if method == Some(&Method::HEAD)
            || method == Some(&Method::CONNECT) && status.is_success()
            || status.is_informational()
        {
            false
        } else {
            !matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED)
        }
    }
    fn can_have_content_length(method: Option<&Method>, status: StatusCode) -> bool {
        if status.is_informational() || method == Some(&Method::CONNECT) && status.is_success() {
            false
        } else {
            !matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED)
        }
    }
    fn can_have_implicit_zero_content_length(method: Option<&Method>, status: StatusCode) -> bool {
        Server::can_have_content_length(method, status) && method != Some(&Method::HEAD)
    }
    fn encode_headers_with_lower_case(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        is_last: bool,
        orig_len: usize,
        wrote_len: bool,
    ) -> crate::hyper::Result<Encoder> {
        struct LowercaseWriter;
        impl HeaderNameWriter for LowercaseWriter {
            #[inline]
            fn write_full_header_line(
                &mut self,
                dst: &mut Vec<u8>,
                line: &str,
                _: (HeaderName, &str),
            ) {
                extend(dst, line.as_bytes());
            }
            #[inline]
            fn write_header_name_with_colon(
                &mut self,
                dst: &mut Vec<u8>,
                name_with_colon: &str,
                _: HeaderName,
            ) {
                extend(dst, name_with_colon.as_bytes());
            }
            #[inline]
            fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName) {
                extend(dst, name.as_str().as_bytes());
            }
        }
        Self::encode_headers(msg, dst, is_last, orig_len, wrote_len, LowercaseWriter)
    }
    #[cold]
    #[inline(never)]
    fn encode_headers_with_original_case(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        is_last: bool,
        orig_len: usize,
        wrote_len: bool,
        orig_headers: &HeaderCaseMap,
    ) -> crate::hyper::Result<Encoder> {
        struct OrigCaseWriter<'map> {
            map: &'map HeaderCaseMap,
            current: Option<(HeaderName, ValueIter<'map, Bytes>)>,
            title_case_headers: bool,
        }
        impl HeaderNameWriter for OrigCaseWriter<'_> {
            #[inline]
            fn write_full_header_line(
                &mut self,
                dst: &mut Vec<u8>,
                _: &str,
                (name, rest): (HeaderName, &str),
            ) {
                self.write_header_name(dst, &name);
                extend(dst, rest.as_bytes());
            }
            #[inline]
            fn write_header_name_with_colon(
                &mut self,
                dst: &mut Vec<u8>,
                _: &str,
                name: HeaderName,
            ) {
                self.write_header_name(dst, &name);
                extend(dst, b": ");
            }
            #[inline]
            fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName) {
                let Self {
                    map,
                    current,
                    title_case_headers,
                } = self;
                if current.as_ref().map_or(true, |(last, _)| last != name) {
                    *current = None;
                }
                let (_, values) =
                    current.get_or_insert_with(|| (name.clone(), map.get_all_internal(name)));
                if let Some(orig_name) = values.next() {
                    extend(dst, orig_name);
                } else if *title_case_headers {
                    title_case(dst, name.as_str().as_bytes());
                } else {
                    extend(dst, name.as_str().as_bytes());
                }
            }
        }
        let header_name_writer = OrigCaseWriter {
            map: orig_headers,
            current: None,
            title_case_headers: msg.title_case_headers,
        };
        Self::encode_headers(msg, dst, is_last, orig_len, wrote_len, header_name_writer)
    }
    #[inline]
    fn encode_headers<W>(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        mut is_last: bool,
        orig_len: usize,
        mut wrote_len: bool,
        mut header_name_writer: W,
    ) -> crate::hyper::Result<Encoder>
    where
        W: HeaderNameWriter,
    {
        let rewind = |dst: &mut Vec<u8>| {
            dst.truncate(orig_len);
        };
        let mut encoder = Encoder::length(0);
        let mut allowed_trailer_fields: Option<Vec<HeaderName>> = None;
        let mut wrote_date = false;
        let mut cur_name = None;
        let mut is_name_written = false;
        let mut must_write_chunked = false;
        let mut prev_con_len = None;
        macro_rules! handle_is_name_written {
            () => {{
                if is_name_written {
                    debug_assert_ne!(
                        &dst[dst.len() - 2..],
                        b"\r\n",
                        "previous header wrote newline but set is_name_written"
                    );
                    if must_write_chunked {
                        extend(dst, b", chunked\r\n");
                    } else {
                        extend(dst, b"\r\n");
                    }
                }
            }};
        }
        'headers: for (opt_name, value) in msg.head.headers.drain() {
            if let Some(n) = opt_name {
                cur_name = Some(n);
                handle_is_name_written!();
                is_name_written = false;
            }
            let name = cur_name.as_ref().expect("current header name");
            match *name {
                header::CONTENT_LENGTH => {
                    if wrote_len && !is_name_written {
                        warn!("unexpected content-length found, canceling");
                        rewind(dst);
                        return Err(crate::hyper::Error::new_user_header());
                    }
                    match msg.body {
                        Some(BodyLength::Known(known_len)) => {
                            #[cfg(debug_assertions)]
                            {
                                if let Some(len) = headers::content_length_parse(&value) {
                                    if msg.req_method != &Some(Method::HEAD) || known_len != 0 {
                                        assert!(
                                            len == known_len,
                                            "payload claims content-length of {known_len}, custom content-length header claims {len}",
                                        );
                                    }
                                }
                            }
                            if !is_name_written {
                                encoder = Encoder::length(known_len);
                                header_name_writer.write_header_name_with_colon(
                                    dst,
                                    "content-length: ",
                                    header::CONTENT_LENGTH,
                                );
                                extend(dst, value.as_bytes());
                                wrote_len = true;
                                is_name_written = true;
                            }
                            continue 'headers;
                        }
                        Some(BodyLength::Unknown) => {
                            if let Some(len) = headers::content_length_parse(&value) {
                                if let Some(prev) = prev_con_len {
                                    if prev != len {
                                        warn!(
                                            "multiple Content-Length values found: [{}, {}]",
                                            prev, len
                                        );
                                        rewind(dst);
                                        return Err(crate::hyper::Error::new_user_header());
                                    }
                                    debug_assert!(is_name_written);
                                    continue 'headers;
                                } else {
                                    encoder = Encoder::length(len);
                                    header_name_writer.write_header_name_with_colon(
                                        dst,
                                        "content-length: ",
                                        header::CONTENT_LENGTH,
                                    );
                                    extend(dst, value.as_bytes());
                                    wrote_len = true;
                                    is_name_written = true;
                                    prev_con_len = Some(len);
                                    continue 'headers;
                                }
                            } else {
                                warn!("illegal Content-Length value: {:?}", value);
                                rewind(dst);
                                return Err(crate::hyper::Error::new_user_header());
                            }
                        }
                        None => {
                            if msg.req_method == &Some(Method::HEAD) {
                                debug_assert_eq!(encoder, Encoder::length(0));
                            } else {
                                if value.as_bytes() != b"0" {
                                    warn!(
                                        "content-length value found, but empty body provided: {:?}",
                                        value
                                    );
                                }
                                continue 'headers;
                            }
                        }
                    }
                    wrote_len = true;
                }
                header::TRANSFER_ENCODING => {
                    if wrote_len && !is_name_written {
                        warn!("unexpected transfer-encoding found, canceling");
                        rewind(dst);
                        return Err(crate::hyper::Error::new_user_header());
                    }
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method.as_ref(), msg.head.subject)
                    {
                        continue;
                    }
                    wrote_len = true;
                    must_write_chunked = !headers::is_chunked_(&value);
                    if !is_name_written {
                        encoder = Encoder::chunked();
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "transfer-encoding: ",
                            header::TRANSFER_ENCODING,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }
                    continue 'headers;
                }
                header::CONNECTION => {
                    if !is_last && headers::connection_close(&value) {
                        is_last = true;
                    }
                    if !is_name_written {
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "connection: ",
                            header::CONNECTION,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }
                    continue 'headers;
                }
                header::DATE => {
                    wrote_date = true;
                }
                header::TRAILER => {
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method.as_ref(), msg.head.subject)
                    {
                        continue;
                    }
                    if !is_name_written {
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "trailer: ",
                            header::TRAILER,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }
                    if let Ok(value_str) = value.to_str() {
                        let names: Vec<HeaderName> = value_str
                            .split(',')
                            .filter_map(|s| HeaderName::from_bytes(s.trim().as_bytes()).ok())
                            .collect();
                        match &mut allowed_trailer_fields {
                            Some(fields) => {
                                fields.extend(names);
                            }
                            None => {
                                allowed_trailer_fields = Some(names);
                            }
                        }
                    }
                    continue 'headers;
                }
                _ => (),
            }
            debug_assert!(
                !is_name_written,
                "{name:?} set is_name_written and didn't continue loop",
            );
            header_name_writer.write_header_name(dst, name);
            extend(dst, b": ");
            extend(dst, value.as_bytes());
            extend(dst, b"\r\n");
        }
        handle_is_name_written!();
        if !wrote_len {
            encoder = match msg.body {
                Some(BodyLength::Unknown) => {
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method.as_ref(), msg.head.subject)
                    {
                        Encoder::close_delimited()
                    } else {
                        header_name_writer.write_full_header_line(
                            dst,
                            "transfer-encoding: chunked\r\n",
                            (header::TRANSFER_ENCODING, ": chunked\r\n"),
                        );
                        Encoder::chunked()
                    }
                }
                None | Some(BodyLength::Known(0)) => {
                    if Server::can_have_implicit_zero_content_length(
                        msg.req_method.as_ref(),
                        msg.head.subject,
                    ) {
                        header_name_writer.write_full_header_line(
                            dst,
                            "content-length: 0\r\n",
                            (header::CONTENT_LENGTH, ": 0\r\n"),
                        );
                    }
                    Encoder::length(0)
                }
                Some(BodyLength::Known(len)) => {
                    if !Server::can_have_content_length(msg.req_method.as_ref(), msg.head.subject) {
                        Encoder::length(0)
                    } else {
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "content-length: ",
                            header::CONTENT_LENGTH,
                        );
                        extend(dst, crate::itoa::Buffer::new().format(len).as_bytes());
                        extend(dst, b"\r\n");
                        Encoder::length(len)
                    }
                }
            };
        }
        if !Server::can_have_body(msg.req_method.as_ref(), msg.head.subject) {
            trace!(
                "server body forced to 0; method={:?}, status={:?}",
                msg.req_method, msg.head.subject
            );
            encoder = Encoder::length(0);
        }
        if !wrote_date && msg.date_header {
            dst.reserve(date::DATE_VALUE_LENGTH + 8);
            header_name_writer.write_header_name_with_colon(dst, "date: ", header::DATE);
            date::extend(dst);
            extend(dst, b"\r\n\r\n");
        } else {
            extend(dst, b"\r\n");
        }
        if encoder.is_chunked() {
            if let Some(allowed_trailer_fields) = allowed_trailer_fields {
                encoder = encoder.into_chunked_with_trailing_fields(allowed_trailer_fields);
            }
        }
        Ok(encoder.set_last(is_last))
    }
    #[inline]
    fn record_path_range(bytes: &[u8], req_path: &str) -> std::ops::Range<usize> {
        let bytes_ptr = bytes.as_ptr() as usize;
        let start = req_path.as_ptr() as usize - bytes_ptr;
        let end = start + req_path.len();
        std::ops::Range { start, end }
    }
}
trait HeaderNameWriter {
    fn write_full_header_line(
        &mut self,
        dst: &mut Vec<u8>,
        line: &str,
        name_value_pair: (HeaderName, &str),
    );
    fn write_header_name_with_colon(
        &mut self,
        dst: &mut Vec<u8>,
        name_with_colon: &str,
        name: HeaderName,
    );
    fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName);
}
impl Http1Transaction for Client {
    type Incoming = StatusCode;
    type Outgoing = RequestLine;
    fn parse(buf: &mut BytesMut, ctx: ParseContext<'_>) -> ParseResult<StatusCode> {
        debug_assert!(!buf.is_empty(), "parse called with empty buf");
        loop {
            let mut headers_indices: SmallVec<[MaybeUninit<HeaderIndices>; DEFAULT_MAX_HEADERS]> =
                match ctx.h1_max_headers {
                    Some(cap) => smallvec ! [MaybeUninit::uninit () ; cap],
                    None => smallvec_inline ! [MaybeUninit::uninit () ; DEFAULT_MAX_HEADERS],
                };
            let (len, status, reason, version, headers_len) = {
                let mut headers: SmallVec<
                    [MaybeUninit<crate::httparse::Header<'_>>; DEFAULT_MAX_HEADERS],
                > = match ctx.h1_max_headers {
                    Some(cap) => smallvec ! [MaybeUninit::uninit () ; cap],
                    None => smallvec_inline ! [MaybeUninit::uninit () ; DEFAULT_MAX_HEADERS],
                };
                trace!(bytes = buf.len(), "Response.parse");
                let mut res = crate::httparse::Response::new(&mut []);
                let bytes = buf.as_ref();
                match ctx.h1_parser_config.parse_response_with_uninit_headers(
                    &mut res,
                    bytes,
                    &mut headers,
                ) {
                    Ok(crate::httparse::Status::Complete(len)) => {
                        trace!("Response.parse Complete({})", len);
                        let status = StatusCode::from_u16(res.code.expect("httparse completed"))?;
                        let reason = {
                            let reason = res.reason.expect("httparse completed");
                            if Some(reason) != status.canonical_reason() {
                                Some(Bytes::copy_from_slice(reason.as_bytes()))
                            } else {
                                None
                            }
                        };
                        let version = if res.version.expect("httparse completed") == 1 {
                            Version::HTTP_11
                        } else {
                            Version::HTTP_10
                        };
                        record_header_indices(bytes, res.headers, &mut headers_indices)?;
                        let headers_len = res.headers.len();
                        (len, status, reason, version, headers_len)
                    }
                    Ok(crate::httparse::Status::Partial) => return Ok(None),
                    Err(crate::httparse::Error::Version) if ctx.h09_responses => {
                        trace!("Response.parse accepted HTTP/0.9 response");
                        (0, StatusCode::OK, None, Version::HTTP_09, 0)
                    }
                    Err(e) => return Err(e.into()),
                }
            };
            let mut slice = buf.split_to(len);
            if ctx
                .h1_parser_config
                .obsolete_multiline_headers_in_responses_are_allowed()
            {
                for header in &mut headers_indices[..headers_len] {
                    let header = unsafe { header.assume_init_mut() };
                    Client::obs_fold_line(&mut slice, header);
                }
            }
            let slice = slice.freeze();
            let mut headers = ctx.cached_headers.take().unwrap_or_default();
            let mut keep_alive = version == Version::HTTP_11;
            let mut header_case_map = if ctx.preserve_header_case {
                Some(HeaderCaseMap::default())
            } else {
                None
            };
            headers.reserve(headers_len);
            for header in &headers_indices[..headers_len] {
                let header = unsafe { header.assume_init_ref() };
                let name = header_name!(&slice[header.name.0..header.name.1]);
                let value = header_value!(slice.slice(header.value.0..header.value.1));
                if let header::CONNECTION = name {
                    if keep_alive {
                        keep_alive = !headers::connection_close(&value);
                    } else {
                        keep_alive = headers::connection_keep_alive(&value);
                    }
                }
                if let Some(header_case_map) = &mut header_case_map {
                    header_case_map.append(&name, slice.slice(header.name.0..header.name.1));
                }
                headers.append(name, value);
            }
            let mut extensions = crate::http::Extensions::default();
            if let Some(header_case_map) = header_case_map {
                extensions.insert(header_case_map);
            }
            if let Some(reason) = reason {
                let reason = crate::hyper::ext::ReasonPhrase::from_bytes_unchecked(reason);
                extensions.insert(reason);
            }
            let head = MessageHead {
                version,
                subject: status,
                headers,
                extensions,
            };
            if let Some((decode, is_upgrade)) = Client::decoder(&head, ctx.req_method)? {
                return Ok(Some(ParsedMessage {
                    head,
                    decode,
                    expect_continue: false,
                    keep_alive: keep_alive && !is_upgrade,
                    wants_upgrade: is_upgrade,
                }));
            }
            if head.subject.is_informational() {
                if let Some(callback) = ctx.on_informational {
                    callback.call(head.into_response(()));
                }
            }
            if buf.is_empty() {
                return Ok(None);
            }
        }
    }
    fn encode(msg: Encode<'_, Self::Outgoing>, dst: &mut Vec<u8>) -> crate::hyper::Result<Encoder> {
        trace!(
            "Client::encode method={:?}, body={:?}",
            msg.head.subject.0, msg.body
        );
        *msg.req_method = Some(msg.head.subject.0.clone());
        let body = Client::set_length(msg.head, msg.body);
        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);
        extend(dst, msg.head.subject.0.as_str().as_bytes());
        extend(dst, b" ");
        let _ = write!(FastWrite(dst), "{} ", msg.head.subject.1);
        match msg.head.version {
            Version::HTTP_10 => extend(dst, b"HTTP/1.0"),
            Version::HTTP_11 => extend(dst, b"HTTP/1.1"),
            Version::HTTP_2 => {
                debug!("request with HTTP2 version coerced to HTTP/1.1");
                extend(dst, b"HTTP/1.1");
            }
            other => panic!("unexpected request version: {other:?}"),
        }
        extend(dst, b"\r\n");
        if let Some(orig_headers) = msg.head.extensions.get::<HeaderCaseMap>() {
            write_headers_original_case(
                &msg.head.headers,
                orig_headers,
                dst,
                msg.title_case_headers,
            );
        } else if msg.title_case_headers {
            write_headers_title_case(&msg.head.headers, dst);
        } else {
            write_headers(&msg.head.headers, dst);
        }
        extend(dst, b"\r\n");
        msg.head.headers.clear();
        Ok(body)
    }
    fn on_error(_err: &crate::hyper::Error) -> Option<MessageHead<Self::Outgoing>> {
        None
    }
    fn is_client() -> bool {
        true
    }
}
impl Client {
    fn decoder(
        inc: &MessageHead<StatusCode>,
        method: &mut Option<Method>,
    ) -> Result<Option<(DecodedLength, bool)>, Parse> {
        match inc.subject.as_u16() {
            101 => {
                return Ok(Some((DecodedLength::ZERO, true)));
            }
            100 | 102..=199 => {
                trace!("ignoring informational response: {}", inc.subject.as_u16());
                return Ok(None);
            }
            204 | 304 => return Ok(Some((DecodedLength::ZERO, false))),
            _ => (),
        }
        match *method {
            Some(Method::HEAD) => {
                return Ok(Some((DecodedLength::ZERO, false)));
            }
            Some(Method::CONNECT) => {
                if let 200..=299 = inc.subject.as_u16() {
                    return Ok(Some((DecodedLength::ZERO, true)));
                }
            }
            Some(_) => {}
            None => {
                trace!("Client::decoder is missing the Method");
            }
        }
        if inc.headers.contains_key(header::TRANSFER_ENCODING) {
            if inc.version == Version::HTTP_10 {
                debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                Err(Parse::transfer_encoding_unexpected())
            } else if headers::transfer_encoding_is_chunked(&inc.headers) {
                Ok(Some((DecodedLength::CHUNKED, false)))
            } else {
                trace!("not chunked, read till eof");
                Ok(Some((DecodedLength::CLOSE_DELIMITED, false)))
            }
        } else if let Some(len) = headers::content_length_parse_all(&inc.headers) {
            Ok(Some((DecodedLength::checked_new(len)?, false)))
        } else if inc.headers.contains_key(header::CONTENT_LENGTH) {
            debug!("illegal Content-Length header");
            Err(Parse::content_length_invalid())
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            Ok(Some((DecodedLength::CLOSE_DELIMITED, false)))
        }
    }
    fn set_length(head: &mut RequestHead, body: Option<BodyLength>) -> Encoder {
        let body = if let Some(body) = body {
            body
        } else {
            head.headers.remove(header::TRANSFER_ENCODING);
            return Encoder::length(0);
        };
        let can_chunked = head.version == Version::HTTP_11;
        let headers = &mut head.headers;
        let existing_con_len = headers::content_length_parse_all(headers);
        let mut should_remove_con_len = false;
        if !can_chunked {
            if headers.remove(header::TRANSFER_ENCODING).is_some() {
                trace!("removing illegal transfer-encoding header");
            }
            return if let Some(len) = existing_con_len {
                Encoder::length(len)
            } else if let BodyLength::Known(len) = body {
                set_content_length(headers, len)
            } else {
                Encoder::length(0)
            };
        }
        let encoder = match headers.entry(header::TRANSFER_ENCODING) {
            Entry::Occupied(te) => {
                should_remove_con_len = true;
                if headers::is_chunked(te.iter()) {
                    Some(Encoder::chunked())
                } else {
                    warn!("user provided transfer-encoding does not end in 'chunked'");
                    headers::add_chunked(te);
                    Some(Encoder::chunked())
                }
            }
            Entry::Vacant(te) => {
                if let Some(len) = existing_con_len {
                    Some(Encoder::length(len))
                } else if let BodyLength::Unknown = body {
                    match head.subject.0 {
                        Method::GET | Method::HEAD | Method::CONNECT => Some(Encoder::length(0)),
                        _ => {
                            te.insert(HeaderValue::from_static("chunked"));
                            Some(Encoder::chunked())
                        }
                    }
                } else {
                    None
                }
            }
        };
        let encoder = encoder.map(|enc| {
            if enc.is_chunked() {
                let allowed_trailer_fields: Vec<HeaderName> = headers
                    .get_all(header::TRAILER)
                    .iter()
                    .filter_map(|hv| hv.to_str().ok())
                    .flat_map(|s| s.split(','))
                    .filter_map(|s| HeaderName::from_bytes(s.trim().as_bytes()).ok())
                    .collect();
                if !allowed_trailer_fields.is_empty() {
                    return enc.into_chunked_with_trailing_fields(allowed_trailer_fields);
                }
            }
            enc
        });
        if let Some(encoder) = encoder {
            if should_remove_con_len && existing_con_len.is_some() {
                headers.remove(header::CONTENT_LENGTH);
            }
            return encoder;
        }
        let len = if let BodyLength::Known(len) = body {
            len
        } else {
            unreachable!("BodyLength::Unknown would set chunked");
        };
        set_content_length(headers, len)
    }
    fn obs_fold_line(all: &mut [u8], idx: &mut HeaderIndices) {
        let buf = &mut all[idx.value.0..idx.value.1];
        let first_nl = match buf.iter().position(|b| *b == b'\n') {
            Some(i) => i,
            None => return,
        };
        fn trim_start(mut s: &[u8]) -> &[u8] {
            while let [first, rest @ ..] = s {
                if first.is_ascii_whitespace() {
                    s = rest;
                } else {
                    break;
                }
            }
            s
        }
        fn trim_end(mut s: &[u8]) -> &[u8] {
            while let [rest @ .., last] = s {
                if last.is_ascii_whitespace() {
                    s = rest;
                } else {
                    break;
                }
            }
            s
        }
        fn trim(s: &[u8]) -> &[u8] {
            trim_start(trim_end(s))
        }
        let mut unfolded = trim_end(&buf[..first_nl]).to_vec();
        for line in buf[first_nl + 1..].split(|b| *b == b'\n') {
            unfolded.push(b' ');
            unfolded.extend_from_slice(trim(line));
        }
        buf[..unfolded.len()].copy_from_slice(&unfolded);
        idx.value.1 = idx.value.0 + unfolded.len();
    }
}
fn set_content_length(headers: &mut HeaderMap, len: u64) -> Encoder {
    if cfg!(debug_assertions) {
        match headers.entry(header::CONTENT_LENGTH) {
            Entry::Occupied(mut cl) => {
                debug_assert!(headers::content_length_parse_all_values(cl.iter()).is_none());
                error!("user provided content-length header was invalid");
                cl.insert(HeaderValue::from(len));
                Encoder::length(len)
            }
            Entry::Vacant(cl) => {
                cl.insert(HeaderValue::from(len));
                Encoder::length(len)
            }
        }
    } else {
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
        Encoder::length(len)
    }
}
#[derive(Clone, Copy)]
struct HeaderIndices {
    name: (usize, usize),
    value: (usize, usize),
}
fn record_header_indices(
    bytes: &[u8],
    headers: &[crate::httparse::Header<'_>],
    indices: &mut [MaybeUninit<HeaderIndices>],
) -> Result<(), crate::hyper::error::Parse> {
    let bytes_ptr = bytes.as_ptr() as usize;
    for (header, indices) in headers.iter().zip(indices.iter_mut()) {
        if header.name.len() >= (1 << 16) {
            debug!("header name larger than 64kb: {:?}", header.name);
            return Err(crate::hyper::error::Parse::TooLarge);
        }
        let name_start = header.name.as_ptr() as usize - bytes_ptr;
        let name_end = name_start + header.name.len();
        let value_start = header.value.as_ptr() as usize - bytes_ptr;
        let value_end = value_start + header.value.len();
        indices.write(HeaderIndices {
            name: (name_start, name_end),
            value: (value_start, value_end),
        });
    }
    Ok(())
}
fn title_case(dst: &mut Vec<u8>, name: &[u8]) {
    dst.reserve(name.len());
    let mut prev = b'-';
    for &(mut c) in name {
        if prev == b'-' {
            c.make_ascii_uppercase();
        }
        dst.push(c);
        prev = c;
    }
}
pub(crate) fn write_headers_title_case(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        title_case(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
    }
}
pub(crate) fn write_headers(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        extend(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
    }
}
#[cold]
fn write_headers_original_case(
    headers: &HeaderMap,
    orig_case: &HeaderCaseMap,
    dst: &mut Vec<u8>,
    title_case_headers: bool,
) {
    for name in headers.keys() {
        let mut names = orig_case.get_all(name);
        for value in headers.get_all(name) {
            if let Some(orig_name) = names.next() {
                extend(dst, orig_name.as_ref());
            } else if title_case_headers {
                title_case(dst, name.as_str().as_bytes());
            } else {
                extend(dst, name.as_str().as_bytes());
            }
            if value.is_empty() {
                extend(dst, b":\r\n");
            } else {
                extend(dst, b": ");
                extend(dst, value.as_bytes());
                extend(dst, b"\r\n");
            }
        }
    }
}
struct FastWrite<'a>(&'a mut Vec<u8>);
impl fmt::Write for FastWrite<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        extend(self.0, s.as_bytes());
        Ok(())
    }
    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        fmt::write(self, args)
    }
}
#[inline]
fn extend(dst: &mut Vec<u8>, data: &[u8]) {
    dst.extend_from_slice(data);
}

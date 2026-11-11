use crate::httparse::iter::Bytes;
use core::mem::MaybeUninit;
use core::{fmt, mem, result, str};
mod iter;
#[macro_use]
mod macros;
mod simd;

#[inline]
fn is_method_token(b: u8) -> bool {
    match b {
        b'A'..=b'Z' => true,
        _ => TOKEN_MAP[b as usize],
    }
}
static URI_MAP: [bool; 256] = byte_map ! (b'!' ..= 0x7e | 0x80 ..= 0xFF);
#[inline]
pub(crate) fn is_uri_token(b: u8) -> bool {
    URI_MAP[b as usize]
}
static TOKEN_MAP: [bool; 256] = byte_map ! (b'A' ..= b'Z' | b'a' ..= b'z' | b'0' ..= b'9' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~');
#[inline]
pub(crate) fn is_header_name_token(b: u8) -> bool {
    TOKEN_MAP[b as usize]
}
static HEADER_VALUE_MAP: [bool; 256] = byte_map ! (b'\t' | b' ' ..= 0x7e | 0x80 ..= 0xFF);
#[inline]
pub(crate) fn is_header_value_token(b: u8) -> bool {
    HEADER_VALUE_MAP[b as usize]
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Error {
    HeaderName,
    HeaderValue,
    NewLine,
    Status,
    Token,
    TooManyHeaders,
    Version,
}
impl Error {
    #[inline]
    fn description_str(&self) -> &'static str {
        match *self {
            Error::HeaderName => "invalid header name",
            Error::HeaderValue => "invalid header value",
            Error::NewLine => "invalid new line",
            Error::Status => "invalid response status",
            Error::Token => "invalid token",
            Error::TooManyHeaders => "too many headers",
            Error::Version => "invalid HTTP version",
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description_str())
    }
}
impl std::error::Error for Error {
    fn description(&self) -> &str {
        self.description_str()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidChunkSize;
impl fmt::Display for InvalidChunkSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid chunk size")
    }
}
pub type Result<T> = result::Result<Status<T>, Error>;
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Status<T> {
    Complete(T),
    Partial,
}
impl<T> Status<T> {
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            Status::Complete(t) => t,
            Status::Partial => panic!("Tried to unwrap Status::Partial"),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ParserConfig {
    allow_spaces_after_header_name_in_responses: bool,
    allow_obsolete_multiline_headers_in_responses: bool,
    allow_multiple_spaces_in_request_line_delimiters: bool,
    allow_multiple_spaces_in_response_status_delimiters: bool,
    allow_space_before_first_header_name: bool,
    ignore_invalid_headers_in_responses: bool,
    ignore_invalid_headers_in_requests: bool,
}
impl ParserConfig {
    pub fn allow_spaces_after_header_name_in_responses(&mut self, value: bool) -> &mut Self {
        self.allow_spaces_after_header_name_in_responses = value;
        self
    }
    pub fn allow_multiple_spaces_in_request_line_delimiters(&mut self, value: bool) -> &mut Self {
        self.allow_multiple_spaces_in_request_line_delimiters = value;
        self
    }

    pub fn allow_multiple_spaces_in_response_status_delimiters(
        &mut self,
        value: bool,
    ) -> &mut Self {
        self.allow_multiple_spaces_in_response_status_delimiters = value;
        self
    }

    pub fn allow_obsolete_multiline_headers_in_responses(&mut self, value: bool) -> &mut Self {
        self.allow_obsolete_multiline_headers_in_responses = value;
        self
    }
    pub fn obsolete_multiline_headers_in_responses_are_allowed(&self) -> bool {
        self.allow_obsolete_multiline_headers_in_responses
    }
    pub fn allow_space_before_first_header_name(&mut self, value: bool) -> &mut Self {
        self.allow_space_before_first_header_name = value;
        self
    }

    pub fn parse_request_with_uninit_headers<'headers, 'buf>(
        &self,
        request: &mut Request<'headers, 'buf>,
        buf: &'buf [u8],
        headers: &'headers mut [MaybeUninit<Header<'buf>>],
    ) -> Result<usize> {
        request.parse_with_config_and_uninit_headers(buf, self, headers)
    }
    pub fn ignore_invalid_headers_in_responses(&mut self, value: bool) -> &mut Self {
        self.ignore_invalid_headers_in_responses = value;
        self
    }
    pub fn ignore_invalid_headers_in_requests(&mut self, value: bool) -> &mut Self {
        self.ignore_invalid_headers_in_requests = value;
        self
    }
    pub fn parse_response<'buf>(
        &self,
        response: &mut Response<'_, 'buf>,
        buf: &'buf [u8],
    ) -> Result<usize> {
        response.parse_with_config(buf, self)
    }
    pub fn parse_response_with_uninit_headers<'headers, 'buf>(
        &self,
        response: &mut Response<'headers, 'buf>,
        buf: &'buf [u8],
        headers: &'headers mut [MaybeUninit<Header<'buf>>],
    ) -> Result<usize> {
        response.parse_with_config_and_uninit_headers(buf, self, headers)
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct Request<'headers, 'buf> {
    pub method: Option<&'buf str>,
    pub path: Option<&'buf str>,
    pub version: Option<u8>,
    pub headers: &'headers mut [Header<'buf>],
}
impl<'h, 'b> Request<'h, 'b> {
    #[inline]
    pub fn new(headers: &'h mut [Header<'b>]) -> Request<'h, 'b> {
        Request {
            method: None,
            path: None,
            version: None,
            headers,
        }
    }
    fn parse_with_config_and_uninit_headers(
        &mut self,
        buf: &'b [u8],
        config: &ParserConfig,
        mut headers: &'h mut [MaybeUninit<Header<'b>>],
    ) -> Result<usize> {
        let orig_len = buf.len();
        let mut bytes = Bytes::new(buf);
        complete!(skip_empty_lines(&mut bytes));
        let method = complete!(parse_method(&mut bytes));
        self.method = Some(method);
        if config.allow_multiple_spaces_in_request_line_delimiters {
            complete!(skip_spaces(&mut bytes));
        }
        self.path = Some(complete!(parse_uri(&mut bytes)));
        if config.allow_multiple_spaces_in_request_line_delimiters {
            complete!(skip_spaces(&mut bytes));
        }
        self.version = Some(complete!(parse_version(&mut bytes)));
        newline!(bytes);
        let len = orig_len - bytes.len();
        let headers_len = complete!(parse_headers_iter_uninit(
            &mut headers,
            &mut bytes,
            &HeaderParserConfig {
                allow_spaces_after_header_name: false,
                allow_obsolete_multiline_headers: false,
                allow_space_before_first_header_name: config.allow_space_before_first_header_name,
                ignore_invalid_headers: config.ignore_invalid_headers_in_requests
            },
        ));
        self.headers = unsafe { assume_init_slice(headers) };
        Ok(Status::Complete(len + headers_len))
    }

    fn parse_with_config(&mut self, buf: &'b [u8], config: &ParserConfig) -> Result<usize> {
        let headers = mem::take(&mut self.headers);
        unsafe {
            let headers: *mut [Header<'_>] = headers;
            let headers = headers as *mut [MaybeUninit<Header<'_>>];
            match self.parse_with_config_and_uninit_headers(buf, config, &mut *headers) {
                Ok(Status::Complete(idx)) => Ok(Status::Complete(idx)),
                other => {
                    self.headers = &mut *(headers as *mut [Header<'_>]);
                    other
                }
            }
        }
    }
    pub fn parse(&mut self, buf: &'b [u8]) -> Result<usize> {
        self.parse_with_config(buf, &Default::default())
    }
}
#[inline]
fn skip_empty_lines(bytes: &mut Bytes<'_>) -> Result<()> {
    loop {
        let b = bytes.peek();
        match b {
            Some(b'\r') => {
                unsafe { bytes.bump() };
                expect ! (bytes . next () == b'\n' => Err (Error::NewLine));
            }
            Some(b'\n') => unsafe {
                bytes.bump();
            },
            Some(..) => {
                bytes.slice();
                return Ok(Status::Complete(()));
            }
            None => return Ok(Status::Partial),
        }
    }
}
#[inline]
fn skip_spaces(bytes: &mut Bytes<'_>) -> Result<()> {
    loop {
        let b = bytes.peek();
        match b {
            Some(b' ') => {
                unsafe { bytes.bump() };
            }
            Some(..) => {
                bytes.slice();
                return Ok(Status::Complete(()));
            }
            None => return Ok(Status::Partial),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct Response<'headers, 'buf> {
    pub version: Option<u8>,
    pub code: Option<u16>,
    pub reason: Option<&'buf str>,
    pub headers: &'headers mut [Header<'buf>],
}
impl<'h, 'b> Response<'h, 'b> {
    #[inline]
    pub fn new(headers: &'h mut [Header<'b>]) -> Response<'h, 'b> {
        Response {
            version: None,
            code: None,
            reason: None,
            headers,
        }
    }
    pub fn parse(&mut self, buf: &'b [u8]) -> Result<usize> {
        self.parse_with_config(buf, &ParserConfig::default())
    }
    fn parse_with_config(&mut self, buf: &'b [u8], config: &ParserConfig) -> Result<usize> {
        let headers = mem::take(&mut self.headers);
        unsafe {
            let headers: *mut [Header<'_>] = headers;
            let headers = headers as *mut [MaybeUninit<Header<'_>>];
            match self.parse_with_config_and_uninit_headers(buf, config, &mut *headers) {
                Ok(Status::Complete(idx)) => Ok(Status::Complete(idx)),
                other => {
                    self.headers = &mut *(headers as *mut [Header<'_>]);
                    other
                }
            }
        }
    }
    fn parse_with_config_and_uninit_headers(
        &mut self,
        buf: &'b [u8],
        config: &ParserConfig,
        mut headers: &'h mut [MaybeUninit<Header<'b>>],
    ) -> Result<usize> {
        let orig_len = buf.len();
        let mut bytes = Bytes::new(buf);
        complete!(skip_empty_lines(&mut bytes));
        self.version = Some(complete!(parse_version(&mut bytes)));
        space ! (bytes or Error::Version);
        if config.allow_multiple_spaces_in_response_status_delimiters {
            complete!(skip_spaces(&mut bytes));
        }
        self.code = Some(complete!(parse_code(&mut bytes)));
        match next!(bytes) {
            b' ' => {
                if config.allow_multiple_spaces_in_response_status_delimiters {
                    complete!(skip_spaces(&mut bytes));
                }
                bytes.slice();
                self.reason = Some(complete!(parse_reason(&mut bytes)));
            }
            b'\r' => {
                expect ! (bytes . next () == b'\n' => Err (Error::Status));
                bytes.slice();
                self.reason = Some("");
            }
            b'\n' => {
                bytes.slice();
                self.reason = Some("");
            }
            _ => return Err(Error::Status),
        }
        let len = orig_len - bytes.len();
        let headers_len = complete!(parse_headers_iter_uninit(
            &mut headers,
            &mut bytes,
            &HeaderParserConfig {
                allow_spaces_after_header_name: config.allow_spaces_after_header_name_in_responses,
                allow_obsolete_multiline_headers: config
                    .allow_obsolete_multiline_headers_in_responses,
                allow_space_before_first_header_name: config.allow_space_before_first_header_name,
                ignore_invalid_headers: config.ignore_invalid_headers_in_responses
            }
        ));
        self.headers = unsafe { assume_init_slice(headers) };
        Ok(Status::Complete(len + headers_len))
    }
}
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Header<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}
impl fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("Header");
        f.field("name", &self.name);
        if let Ok(value) = str::from_utf8(self.value) {
            f.field("value", &value);
        } else {
            f.field("value", &self.value);
        }
        f.finish()
    }
}
pub const EMPTY_HEADER: Header<'static> = Header {
    name: "",
    value: b"",
};
#[inline]
pub fn parse_version(bytes: &mut Bytes) -> Result<u8> {
    if let Some(eight) = bytes.peek_n::<[u8; 8]>(8) {
        let h10: u64 = u64::from_ne_bytes(*b"HTTP/1.0");
        let h11: u64 = u64::from_ne_bytes(*b"HTTP/1.1");
        unsafe {
            bytes.advance(8);
        }
        let block = u64::from_ne_bytes(eight);
        return if block == h10 {
            Ok(Status::Complete(0))
        } else if block == h11 {
            Ok(Status::Complete(1))
        } else {
            Err(Error::Version)
        };
    }
    expect ! (bytes . next () == b'H' => Err (Error::Version));
    expect ! (bytes . next () == b'T' => Err (Error::Version));
    expect ! (bytes . next () == b'T' => Err (Error::Version));
    expect ! (bytes . next () == b'P' => Err (Error::Version));
    expect ! (bytes . next () == b'/' => Err (Error::Version));
    expect ! (bytes . next () == b'1' => Err (Error::Version));
    expect ! (bytes . next () == b'.' => Err (Error::Version));
    Ok(Status::Partial)
}
#[inline]
pub fn parse_method<'a>(bytes: &mut Bytes<'a>) -> Result<&'a str> {
    const GET: [u8; 4] = *b"GET ";
    const POST: [u8; 4] = *b"POST";
    match bytes.peek_n::<[u8; 4]>(4) {
        Some(GET) => {
            let method = unsafe {
                bytes.advance(4);
                str::from_utf8_unchecked(bytes.slice_skip(1))
            };
            Ok(Status::Complete(method))
        }
        Some(POST) if unsafe { bytes.peek_ahead(4) } == Some(b' ') => {
            let method = unsafe {
                bytes.advance(5);
                str::from_utf8_unchecked(bytes.slice_skip(1))
            };
            Ok(Status::Complete(method))
        }
        _ => parse_token(bytes),
    }
}
#[inline]
fn parse_reason<'a>(bytes: &mut Bytes<'a>) -> Result<&'a str> {
    let mut seen_obs_text = false;
    loop {
        let b = next!(bytes);
        if b == b'\r' {
            expect ! (bytes . next () == b'\n' => Err (Error::Status));
            return Ok(Status::Complete(unsafe {
                let bytes = bytes.slice_skip(2);
                if !seen_obs_text {
                    str::from_utf8_unchecked(bytes)
                } else {
                    ""
                }
            }));
        } else if b == b'\n' {
            return Ok(Status::Complete(unsafe {
                let bytes = bytes.slice_skip(1);
                if !seen_obs_text {
                    str::from_utf8_unchecked(bytes)
                } else {
                    ""
                }
            }));
        } else if !(b == 0x09 || b == b' ' || (0x21..=0x7E).contains(&b) || b >= 0x80) {
            return Err(Error::Status);
        } else if b >= 0x80 {
            seen_obs_text = true;
        }
    }
}
#[inline]
fn parse_token<'a>(bytes: &mut Bytes<'a>) -> Result<&'a str> {
    let b = next!(bytes);
    if !is_method_token(b) {
        return Err(Error::Token);
    }
    loop {
        let b = next!(bytes);
        if b == b' ' {
            return Ok(Status::Complete(unsafe {
                str::from_utf8_unchecked(bytes.slice_skip(1))
            }));
        } else if !is_method_token(b) {
            return Err(Error::Token);
        }
    }
}
#[inline]
pub fn parse_uri<'a>(bytes: &mut Bytes<'a>) -> Result<&'a str> {
    let start = bytes.pos();
    simd::match_uri_vectored(bytes);
    let end = bytes.pos();
    if next!(bytes) == b' ' {
        if end == start {
            return Err(Error::Token);
        }
        match str::from_utf8(unsafe { bytes.slice_skip(1) }) {
            Ok(uri) => Ok(Status::Complete(uri)),
            Err(_) => Err(Error::Token),
        }
    } else {
        Err(Error::Token)
    }
}
#[inline]
fn parse_code(bytes: &mut Bytes<'_>) -> Result<u16> {
    let hundreds = expect ! (bytes . next () == b'0' ..= b'9' => Err (Error::Status));
    let tens = expect ! (bytes . next () == b'0' ..= b'9' => Err (Error::Status));
    let ones = expect ! (bytes . next () == b'0' ..= b'9' => Err (Error::Status));
    Ok(Status::Complete(
        (hundreds - b'0') as u16 * 100 + (tens - b'0') as u16 * 10 + (ones - b'0') as u16,
    ))
}
pub fn parse_headers<'b: 'h, 'h>(
    src: &'b [u8],
    mut dst: &'h mut [Header<'b>],
) -> Result<(usize, &'h [Header<'b>])> {
    let mut iter = Bytes::new(src);
    let pos = complete!(parse_headers_iter(
        &mut dst,
        &mut iter,
        &HeaderParserConfig::default()
    ));
    Ok(Status::Complete((pos, dst)))
}
#[inline]
fn parse_headers_iter<'a>(
    headers: &mut &mut [Header<'a>],
    bytes: &mut Bytes<'a>,
    config: &HeaderParserConfig,
) -> Result<usize> {
    parse_headers_iter_uninit(unsafe { deinit_slice_mut(headers) }, bytes, config)
}
unsafe fn deinit_slice_mut<'a, 'b, T>(s: &'a mut &'b mut [T]) -> &'a mut &'b mut [MaybeUninit<T>] {
    unsafe {
        let s: *mut &mut [T] = s;
        let s = s as *mut &mut [MaybeUninit<T>];
        &mut *s
    }
}
unsafe fn assume_init_slice<T>(s: &mut [MaybeUninit<T>]) -> &mut [T] {
    unsafe {
        let s: *mut [MaybeUninit<T>] = s;
        let s = s as *mut [T];
        &mut *s
    }
}
#[derive(Clone, Debug, Default)]
struct HeaderParserConfig {
    allow_spaces_after_header_name: bool,
    allow_obsolete_multiline_headers: bool,
    allow_space_before_first_header_name: bool,
    ignore_invalid_headers: bool,
}
fn parse_headers_iter_uninit<'a>(
    headers: &mut &mut [MaybeUninit<Header<'a>>],
    bytes: &mut Bytes<'a>,
    config: &HeaderParserConfig,
) -> Result<usize> {
    struct ShrinkOnDrop<'r1, 'r2, 'a> {
        headers: &'r1 mut &'r2 mut [MaybeUninit<Header<'a>>],
        num_headers: usize,
    }
    impl Drop for ShrinkOnDrop<'_, '_, '_> {
        fn drop(&mut self) {
            let headers = mem::take(self.headers);
            let headers = unsafe { headers.get_unchecked_mut(..self.num_headers) };
            *self.headers = headers;
        }
    }
    let mut autoshrink = ShrinkOnDrop {
        headers,
        num_headers: 0,
    };
    let start = bytes.as_ref().as_ptr() as usize;
    let mut result = Err(Error::TooManyHeaders);
    let mut iter = autoshrink.headers.iter_mut();
    macro_rules! maybe_continue_after_obsolete_line_folding { ($bytes : ident , $label : lifetime) => { if config . allow_obsolete_multiline_headers { match $bytes . peek () { None => { return Ok (Status::Partial) ; } Some (b' ') | Some (b'\t') => { continue $label ; } _ => { } , } } } }
    'headers: loop {
        macro_rules! handle_invalid_char { ($bytes : ident , $b : ident , $err : ident) => { if ! config . ignore_invalid_headers { return Err (Error::$err) ; } let mut b = $b ; loop { if b == b'\r' { expect ! (bytes . next () == b'\n' => Err (Error::$err)) ; break ; } if b == b'\n' { break ; } if b == b'\0' { return Err (Error::$err) ; } b = next ! ($bytes) ; } $bytes . slice () ; continue 'headers ; } ; }
        let b = next!(bytes);
        if b == b'\r' {
            expect ! (bytes . next () == b'\n' => Err (Error::NewLine));
            let end = bytes.as_ref().as_ptr() as usize;
            result = Ok(Status::Complete(end - start));
            break;
        }
        if b == b'\n' {
            let end = bytes.as_ref().as_ptr() as usize;
            result = Ok(Status::Complete(end - start));
            break;
        }
        if !is_header_name_token(b) {
            if config.allow_space_before_first_header_name
                && autoshrink.num_headers == 0
                && (b == b' ' || b == b'\t')
            {
                while let Some(peek) = bytes.peek() {
                    if peek == b' ' || peek == b'\t' {
                        next!(bytes);
                    } else {
                        break;
                    }
                }
                bytes.slice();
                continue 'headers;
            } else {
                handle_invalid_char!(bytes, b, HeaderName);
            }
        }
        let header_name: &str = 'name: loop {
            simd::match_header_name_vectored(bytes);
            let mut b = next!(bytes);
            let bslice = unsafe { bytes.slice_skip(1) };
            let name = unsafe { str::from_utf8_unchecked(bslice) };
            if b == b':' {
                break 'name name;
            }
            if config.allow_spaces_after_header_name {
                while b == b' ' || b == b'\t' {
                    b = next!(bytes);
                    if b == b':' {
                        bytes.slice();
                        break 'name name;
                    }
                }
            }
            handle_invalid_char!(bytes, b, HeaderName);
        };
        let mut b;
        let value_slice = 'value: loop {
            'whitespace_after_colon: loop {
                b = next!(bytes);
                if b == b' ' || b == b'\t' {
                    bytes.slice();
                    continue 'whitespace_after_colon;
                }
                if is_header_value_token(b) {
                    break 'whitespace_after_colon;
                }
                if b == b'\r' {
                    expect ! (bytes . next () == b'\n' => Err (Error::HeaderValue));
                } else if b != b'\n' {
                    handle_invalid_char!(bytes, b, HeaderValue);
                }
                maybe_continue_after_obsolete_line_folding ! (bytes , 'whitespace_after_colon);
                let whitespace_slice = bytes.slice();
                break 'value &whitespace_slice[0..0];
            }
            'value_lines: loop {
                simd::match_header_value_vectored(bytes);
                let b = next!(bytes);
                let skip = if b == b'\r' {
                    expect ! (bytes . next () == b'\n' => Err (Error::HeaderValue));
                    2
                } else if b == b'\n' {
                    1
                } else {
                    handle_invalid_char!(bytes, b, HeaderValue);
                };
                maybe_continue_after_obsolete_line_folding ! (bytes , 'value_lines);
                unsafe {
                    break 'value bytes.slice_skip(skip);
                }
            }
        };
        let uninit_header = match iter.next() {
            Some(header) => header,
            None => break 'headers,
        };
        let header_value = if let Some(last_visible) = value_slice
            .iter()
            .rposition(|b| *b != b' ' && *b != b'\t' && *b != b'\r' && *b != b'\n')
        {
            &value_slice[0..last_visible + 1]
        } else {
            value_slice
        };
        *uninit_header = MaybeUninit::new(Header {
            name: header_name,
            value: header_value,
        });
        autoshrink.num_headers += 1;
    }
    result
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/httparse.rs"]
mod tests;

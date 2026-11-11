use crate::http::{HeaderName, Uri, request::Parts};
use crate::tungstenite::{
    error::{Error, Result, UrlError},
    handshake::{HandshakeError, client::ClientHandshake},
    protocol::WebSocket,
    stream::Mode,
};
use crate::tungstenite::{
    handshake::client::{Request, Response, generate_key},
    protocol::WebSocketConfig,
};
use log::*;
use std::{
    io::{Read, Write},
    result::Result as StdResult,
};

pub fn uri_mode(uri: &Uri) -> Result<Mode> {
    match uri.scheme_str() {
        Some("ws") => Ok(Mode::Plain),
        Some("wss") => Ok(Mode::Tls),
        _ => Err(Error::Url(UrlError::UnsupportedUrlScheme)),
    }
}
pub fn client_with_config<Stream, Req>(
    request: Req,
    stream: Stream,
    config: Option<WebSocketConfig>,
) -> StdResult<(WebSocket<Stream>, Response), HandshakeError<ClientHandshake<Stream>>>
where
    Stream: Read + Write,
    Req: IntoClientRequest,
{
    ClientHandshake::start(stream, request.into_client_request()?, config)?.handshake()
}
pub fn client<Stream, Req>(
    request: Req,
    stream: Stream,
) -> StdResult<(WebSocket<Stream>, Response), HandshakeError<ClientHandshake<Stream>>>
where
    Stream: Read + Write,
    Req: IntoClientRequest,
{
    client_with_config(request, stream, None)
}
pub trait IntoClientRequest {
    fn into_client_request(self) -> Result<Request>;
}
impl IntoClientRequest for &str {
    fn into_client_request(self) -> Result<Request> {
        self.parse::<Uri>()?.into_client_request()
    }
}
impl IntoClientRequest for &String {
    fn into_client_request(self) -> Result<Request> {
        <&str as IntoClientRequest>::into_client_request(self)
    }
}
impl IntoClientRequest for String {
    fn into_client_request(self) -> Result<Request> {
        <&str as IntoClientRequest>::into_client_request(&self)
    }
}
impl IntoClientRequest for &Uri {
    fn into_client_request(self) -> Result<Request> {
        self.clone().into_client_request()
    }
}
impl IntoClientRequest for Uri {
    fn into_client_request(self) -> Result<Request> {
        let authority = self
            .authority()
            .ok_or(Error::Url(UrlError::NoHostName))?
            .as_str();
        let host = authority
            .find('@')
            .map(|idx| authority.split_at(idx + 1).1)
            .unwrap_or_else(|| authority);
        if host.is_empty() {
            return Err(Error::Url(UrlError::EmptyHostName));
        }
        let req = Request::builder()
            .method("GET")
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .uri(self)
            .body(())?;
        Ok(req)
    }
}
impl IntoClientRequest for Request {
    fn into_client_request(self) -> Result<Request> {
        Ok(self)
    }
}
impl IntoClientRequest for crate::httparse::Request<'_, '_> {
    fn into_client_request(self) -> Result<Request> {
        use crate::tungstenite::handshake::headers::FromHttparse;
        Request::from_httparse(self)
    }
}
#[derive(Debug, Clone)]
pub struct ClientRequestBuilder {
    uri: Uri,
    additional_headers: Vec<(String, String)>,
    subprotocols: Vec<String>,
}
impl ClientRequestBuilder {
    #[must_use]
    pub const fn new(uri: Uri) -> Self {
        Self {
            uri,
            additional_headers: Vec::new(),
            subprotocols: Vec::new(),
        }
    }
}
impl IntoClientRequest for ClientRequestBuilder {
    fn into_client_request(self) -> Result<Request> {
        let mut request = self.uri.into_client_request()?;
        let headers = request.headers_mut();
        for (k, v) in self.additional_headers {
            let key = HeaderName::try_from(k)?;
            let value = v.parse()?;
            headers.append(key, value);
        }
        if !self.subprotocols.is_empty() {
            let protocols = self.subprotocols.join(", ").parse()?;
            headers.append("Sec-WebSocket-Protocol", protocols);
        }
        Ok(request)
    }
}

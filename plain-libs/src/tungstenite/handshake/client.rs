use super::{
    HandshakeRole, MidHandshake, ProcessingResult, derive_accept_key,
    headers::{FromHttparse, MAX_HEADERS},
    machine::{HandshakeMachine, StageResult, TryParse},
};
use crate::http::{
    HeaderMap, Request as HttpRequest, Response as HttpResponse, StatusCode, header::HeaderName,
};
use crate::httparse::Status;
use crate::tungstenite::{
    error::{Error, ProtocolError, Result, SubProtocolError, UrlError},
    handshake::version_as_str,
    protocol::{Role, WebSocket, WebSocketConfig},
};
use log::*;
use std::{
    io::{Read, Write},
    marker::PhantomData,
};
pub type Request = HttpRequest<()>;
pub type Response = HttpResponse<Option<Vec<u8>>>;
#[derive(Debug)]
pub struct ClientHandshake<S> {
    verify_data: VerifyData,
    config: Option<WebSocketConfig>,
    _marker: PhantomData<S>,
}
impl<S: Read + Write> ClientHandshake<S> {
    pub fn start(
        stream: S,
        request: Request,
        config: Option<WebSocketConfig>,
    ) -> Result<MidHandshake<Self>> {
        if request.method() != crate::http::Method::GET {
            return Err(Error::Protocol(ProtocolError::WrongHttpMethod));
        }
        if request.version() < crate::http::Version::HTTP_11 {
            return Err(Error::Protocol(ProtocolError::WrongHttpVersion));
        }
        let _ = crate::tungstenite::client::uri_mode(request.uri())?;
        let subprotocols = extract_subprotocols_from_request(&request)?;
        let (request, key) = generate_request(request)?;
        let machine = HandshakeMachine::start_write(stream, request);
        let client = {
            let accept_key = derive_accept_key(key.as_ref());
            ClientHandshake {
                verify_data: VerifyData {
                    accept_key,
                    subprotocols,
                },
                config,
                _marker: PhantomData,
            }
        };
        trace!("Client handshake initiated.");
        Ok(MidHandshake {
            role: client,
            machine,
        })
    }
}
impl<S: Read + Write> HandshakeRole for ClientHandshake<S> {
    type IncomingData = Response;
    type InternalStream = S;
    type FinalResult = (WebSocket<S>, Response);
    fn stage_finished(
        &mut self,
        finish: StageResult<Self::IncomingData, Self::InternalStream>,
    ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>> {
        Ok(match finish {
            StageResult::DoneWriting(stream) => {
                ProcessingResult::Continue(HandshakeMachine::start_read(stream))
            }
            StageResult::DoneReading {
                stream,
                result,
                tail,
            } => {
                let result = match self.verify_data.verify_response(result) {
                    Ok(r) => r,
                    Err(Error::Http(mut e)) => {
                        *e.body_mut() = Some(tail);
                        return Err(Error::Http(e));
                    }
                    Err(e) => return Err(e),
                };
                debug!("Client handshake done.");
                let websocket =
                    WebSocket::from_partially_read(stream, tail, Role::Client, self.config);
                ProcessingResult::Done((websocket, result))
            }
        })
    }
}
pub fn generate_request(mut request: Request) -> Result<(Vec<u8>, String)> {
    let mut req = Vec::new();
    write!(
        req,
        "GET {path} {version}\r\n",
        path = request
            .uri()
            .path_and_query()
            .ok_or(Error::Url(UrlError::NoPathOrQuery))?
            .as_str(),
        version = version_as_str(request.version())?,
    )
    .unwrap();
    const KEY_HEADERNAME: &str = "Sec-WebSocket-Key";
    const WEBSOCKET_HEADERS: [&str; 5] = [
        "Host",
        "Connection",
        "Upgrade",
        "Sec-WebSocket-Version",
        KEY_HEADERNAME,
    ];
    let key = request
        .headers()
        .get(KEY_HEADERNAME)
        .ok_or_else(|| {
            Error::Protocol(ProtocolError::InvalidHeader(
                HeaderName::from_bytes(KEY_HEADERNAME.as_bytes())
                    .unwrap()
                    .into(),
            ))
        })?
        .to_str()?
        .to_owned();
    let headers = request.headers_mut();
    for &header in &WEBSOCKET_HEADERS {
        let value = headers.remove(header).ok_or_else(|| {
            Error::Protocol(ProtocolError::InvalidHeader(
                HeaderName::from_bytes(header.as_bytes()).unwrap().into(),
            ))
        })?;
        write!(
            req,
            "{header}: {value}\r\n",
            header = header,
            value = value.to_str().map_err(|err| {
                Error::Utf8(format!(
                    "{err} for header name '{header}' with value: {value:?}"
                ))
            })?
        )
        .unwrap();
    }
    let websocket_headers_contains = |name| {
        WEBSOCKET_HEADERS
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name))
    };
    for (k, v) in headers {
        let mut name = k.as_str();
        if websocket_headers_contains(name) {
            return Err(Error::Protocol(ProtocolError::InvalidHeader(
                k.clone().into(),
            )));
        }
        if name == "sec-websocket-protocol" {
            name = "Sec-WebSocket-Protocol";
        }
        if name == "origin" {
            name = "Origin";
        }
        req.extend_from_slice(name.as_bytes());
        req.extend_from_slice(b": ");
        req.extend_from_slice(v.as_bytes());
        req.extend_from_slice(b"\r\n");
    }
    req.extend_from_slice(b"\r\n");
    trace!("Request: {:?}", String::from_utf8_lossy(&req));
    Ok((req, key))
}
fn extract_subprotocols_from_request(request: &Request) -> Result<Option<Vec<String>>> {
    if let Some(subprotocols) = request.headers().get("Sec-WebSocket-Protocol") {
        Ok(Some(
            subprotocols
                .to_str()?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        ))
    } else {
        Ok(None)
    }
}
#[derive(Debug)]
struct VerifyData {
    accept_key: String,
    subprotocols: Option<Vec<String>>,
}
impl VerifyData {
    pub fn verify_response(&self, response: Response) -> Result<Response> {
        if response.status() != StatusCode::SWITCHING_PROTOCOLS {
            return Err(Error::Http(response.into()));
        }
        let headers = response.headers();
        if !headers
            .get("Upgrade")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
        {
            return Err(Error::Protocol(
                ProtocolError::MissingUpgradeWebSocketHeader,
            ));
        }
        if !headers
            .get("Connection")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.eq_ignore_ascii_case("Upgrade"))
            .unwrap_or(false)
        {
            return Err(Error::Protocol(
                ProtocolError::MissingConnectionUpgradeHeader,
            ));
        }
        if !headers
            .get("Sec-WebSocket-Accept")
            .map(|h| h == &self.accept_key)
            .unwrap_or(false)
        {
            return Err(Error::Protocol(
                ProtocolError::SecWebSocketAcceptKeyMismatch,
            ));
        }
        if headers.get("Sec-WebSocket-Protocol").is_none() && self.subprotocols.is_some() {
            return Err(Error::Protocol(
                ProtocolError::SecWebSocketSubProtocolError(SubProtocolError::NoSubProtocol),
            ));
        }
        if headers.get("Sec-WebSocket-Protocol").is_some() && self.subprotocols.is_none() {
            return Err(Error::Protocol(
                ProtocolError::SecWebSocketSubProtocolError(
                    SubProtocolError::ServerSentSubProtocolNoneRequested,
                ),
            ));
        }
        if let Some(returned_subprotocol) = headers.get("Sec-WebSocket-Protocol") {
            if let Some(accepted_subprotocols) = &self.subprotocols {
                if !accepted_subprotocols.contains(&returned_subprotocol.to_str()?.to_string()) {
                    return Err(Error::Protocol(
                        ProtocolError::SecWebSocketSubProtocolError(
                            SubProtocolError::InvalidSubProtocol,
                        ),
                    ));
                }
            }
        }
        Ok(response)
    }
}
impl TryParse for Response {
    fn try_parse(buf: &[u8]) -> Result<Option<(usize, Self)>> {
        let mut hbuffer = [crate::httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut req = crate::httparse::Response::new(&mut hbuffer);
        Ok(match req.parse(buf)? {
            Status::Partial => None,
            Status::Complete(size) => Some((size, Response::from_httparse(req)?)),
        })
    }
}
impl<'h, 'b: 'h> FromHttparse<crate::httparse::Response<'h, 'b>> for Response {
    fn from_httparse(raw: crate::httparse::Response<'h, 'b>) -> Result<Self> {
        if raw.version.expect("Bug: no HTTP version") < 1 {
            return Err(Error::Protocol(ProtocolError::WrongHttpVersion));
        }
        let headers = HeaderMap::from_httparse(raw.headers)?;
        let mut response = Response::new(None);
        *response.status_mut() = StatusCode::from_u16(raw.code.expect("Bug: no HTTP status code"))?;
        *response.headers_mut() = headers;
        *response.version_mut() = crate::http::Version::HTTP_11;
        Ok(response)
    }
}
pub fn generate_key() -> String {
    let r: [u8; 16] = rand::random();
    crate::utils::base64::base64_encode(&r)
}

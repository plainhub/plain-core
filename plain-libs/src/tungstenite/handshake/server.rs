use super::{
    HandshakeRole, MidHandshake, ProcessingResult, derive_accept_key,
    headers::{FromHttparse, MAX_HEADERS},
    machine::{HandshakeMachine, StageResult, TryParse},
};
use crate::http::{
    HeaderMap, Request as HttpRequest, Response as HttpResponse, StatusCode, header::HeaderValue,
    response::Builder,
};
use crate::httparse::Status;
use crate::tungstenite::{
    error::{Error, ProtocolError, Result},
    handshake::version_as_str,
    protocol::{Role, WebSocket, WebSocketConfig},
};
use log::*;
use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    result::Result as StdResult,
};
pub type Request = HttpRequest<()>;
pub type Response = HttpResponse<()>;
pub type ErrorResponse = HttpResponse<Option<String>>;
fn create_parts<T>(request: &HttpRequest<T>) -> Result<Builder> {
    if request.method() != crate::http::Method::GET {
        return Err(Error::Protocol(ProtocolError::WrongHttpMethod));
    }
    if request.version() < crate::http::Version::HTTP_11 {
        return Err(Error::Protocol(ProtocolError::WrongHttpVersion));
    }
    if !request
        .headers()
        .get("Connection")
        .and_then(|h| h.to_str().ok())
        .map(|h| {
            h.split([' ', ','])
                .any(|p| p.eq_ignore_ascii_case("Upgrade"))
        })
        .unwrap_or(false)
    {
        return Err(Error::Protocol(
            ProtocolError::MissingConnectionUpgradeHeader,
        ));
    }
    if !request
        .headers()
        .get("Upgrade")
        .and_then(|h| h.to_str().ok())
        .map(|h| h.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
    {
        return Err(Error::Protocol(
            ProtocolError::MissingUpgradeWebSocketHeader,
        ));
    }
    if !request
        .headers()
        .get("Sec-WebSocket-Version")
        .map(|h| h == "13")
        .unwrap_or(false)
    {
        return Err(Error::Protocol(
            ProtocolError::MissingSecWebSocketVersionHeader,
        ));
    }
    let key = request
        .headers()
        .get("Sec-WebSocket-Key")
        .ok_or(Error::Protocol(ProtocolError::MissingSecWebSocketKey))?;
    if !is_valid_sec_websocket_key(key) {
        return Err(Error::Protocol(ProtocolError::InvalidSecWebSocketKey));
    }
    let builder = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .version(request.version())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Accept", derive_accept_key(key.as_bytes()));
    Ok(builder)
}
fn is_valid_sec_websocket_key(key: &HeaderValue) -> bool {
    if key.len() != 24 {
        return false;
    }
    let Ok(decoded) = crate::utils::base64::base64_decode_standard(key.as_bytes()) else {
        return false;
    };
    decoded.len() == 16
}
pub fn create_response(request: &Request) -> Result<Response> {
    Ok(create_parts(request)?.body(())?)
}

pub fn write_response<T>(mut w: impl io::Write, response: &HttpResponse<T>) -> Result<()> {
    writeln!(
        w,
        "{version} {status}\r",
        version = version_as_str(response.version())?,
        status = response.status()
    )?;
    for (k, v) in response.headers() {
        writeln!(w, "{}: {}\r", k, v.to_str()?)?;
    }
    writeln!(w, "\r")?;
    Ok(())
}
impl TryParse for Request {
    fn try_parse(buf: &[u8]) -> Result<Option<(usize, Self)>> {
        let mut hbuffer = [crate::httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut req = crate::httparse::Request::new(&mut hbuffer);
        Ok(match req.parse(buf)? {
            Status::Partial => None,
            Status::Complete(size) => Some((size, Request::from_httparse(req)?)),
        })
    }
}
impl<'h, 'b: 'h> FromHttparse<crate::httparse::Request<'h, 'b>> for Request {
    fn from_httparse(raw: crate::httparse::Request<'h, 'b>) -> Result<Self> {
        if raw.method.expect("Bug: no method in header") != "GET" {
            return Err(Error::Protocol(ProtocolError::WrongHttpMethod));
        }
        if raw.version.expect("Bug: no HTTP version") < 1 {
            return Err(Error::Protocol(ProtocolError::WrongHttpVersion));
        }
        let headers = HeaderMap::from_httparse(raw.headers)?;
        let mut request = Request::new(());
        *request.method_mut() = crate::http::Method::GET;
        *request.headers_mut() = headers;
        *request.uri_mut() = raw.path.expect("Bug: no path in header").parse()?;
        *request.version_mut() = crate::http::Version::HTTP_11;
        Ok(request)
    }
}
pub trait Callback: Sized {
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> StdResult<Response, ErrorResponse>;
}
impl<F> Callback for F
where
    F: FnOnce(&Request, Response) -> StdResult<Response, ErrorResponse>,
{
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> StdResult<Response, ErrorResponse> {
        self(request, response)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct NoCallback;
impl Callback for NoCallback {
    fn on_request(
        self,
        _request: &Request,
        response: Response,
    ) -> StdResult<Response, ErrorResponse> {
        Ok(response)
    }
}
#[derive(Debug)]
pub struct ServerHandshake<S, C> {
    callback: Option<C>,
    config: Option<WebSocketConfig>,
    error_response: Option<ErrorResponse>,
    _marker: PhantomData<S>,
}
impl<S: Read + Write, C: Callback> ServerHandshake<S, C> {
    pub fn start(stream: S, callback: C, config: Option<WebSocketConfig>) -> MidHandshake<Self> {
        trace!("Server handshake initiated.");
        MidHandshake {
            machine: HandshakeMachine::start_read(stream),
            role: ServerHandshake {
                callback: Some(callback),
                config,
                error_response: None,
                _marker: PhantomData,
            },
        }
    }
}
impl<S: Read + Write, C: Callback> HandshakeRole for ServerHandshake<S, C> {
    type IncomingData = Request;
    type InternalStream = S;
    type FinalResult = WebSocket<S>;
    fn stage_finished(
        &mut self,
        finish: StageResult<Self::IncomingData, Self::InternalStream>,
    ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>> {
        Ok(match finish {
            StageResult::DoneReading {
                stream,
                result,
                tail,
            } => {
                if !tail.is_empty() {
                    return Err(Error::Protocol(ProtocolError::JunkAfterRequest));
                }
                let response = create_response(&result)?;
                let callback_result = if let Some(callback) = self.callback.take() {
                    callback.on_request(&result, response)
                } else {
                    Ok(response)
                };
                match callback_result {
                    Ok(response) => {
                        let mut output = vec![];
                        write_response(&mut output, &response)?;
                        ProcessingResult::Continue(HandshakeMachine::start_write(stream, output))
                    }
                    Err(resp) => {
                        if resp.status().is_success() {
                            return Err(Error::Protocol(ProtocolError::CustomResponseSuccessful));
                        }
                        self.error_response = Some(resp);
                        let resp = self.error_response.as_ref().unwrap();
                        let mut output = vec![];
                        write_response(&mut output, resp)?;
                        if let Some(body) = resp.body() {
                            output.extend_from_slice(body.as_bytes());
                        }
                        ProcessingResult::Continue(HandshakeMachine::start_write(stream, output))
                    }
                }
            }
            StageResult::DoneWriting(stream) => {
                if let Some(err) = self.error_response.take() {
                    debug!("Server handshake failed.");
                    let (parts, body) = err.into_parts();
                    let body = body.map(|b| b.as_bytes().to_vec());
                    return Err(Error::Http(
                        crate::http::Response::from_parts(parts, body).into(),
                    ));
                } else {
                    debug!("Server handshake done.");
                    let websocket = WebSocket::from_raw_socket(stream, Role::Server, self.config);
                    ProcessingResult::Done(websocket)
                }
            }
        })
    }
}

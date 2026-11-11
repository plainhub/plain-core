mod errors;
pub use errors::*;
pub(super) mod messages;
use super::{Handshaking, SocksError};
use crate::http::Uri;
use crate::hyper::rt::{Read, Write};
use bytes::BytesMut;
use messages::*;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::task::{Context, Poll};
use tower_service::Service;
#[derive(Debug, Clone)]
pub struct SocksV5<C> {
    inner: C,
    config: SocksConfig,
}
#[derive(Debug, Clone)]
pub struct SocksConfig {
    proxy: Uri,
    proxy_auth: Option<(String, String)>,
    local_dns: bool,
    optimistic: bool,
}
#[derive(Debug)]
enum State {
    SendingNegReq,
    ReadingNegRes,
    SendingAuthReq,
    ReadingAuthRes,
    SendingProxyReq,
    ReadingProxyRes,
}
impl<C> SocksV5<C> {
    pub fn new(proxy_dst: Uri, connector: C) -> Self {
        Self {
            inner: connector,
            config: SocksConfig::new(proxy_dst),
        }
    }
    pub fn with_auth(mut self, user: String, pass: String) -> Self {
        self.config.proxy_auth = Some((user, pass));
        self
    }
    pub fn local_dns(mut self, local_dns: bool) -> Self {
        self.config.local_dns = local_dns;
        self
    }
}
impl SocksConfig {
    fn new(proxy: Uri) -> Self {
        Self {
            proxy,
            proxy_auth: None,
            local_dns: false,
            optimistic: false,
        }
    }
    async fn execute<T, E>(self, mut conn: T, host: String, port: u16) -> Result<T, SocksError<E>>
    where
        T: Read + Write + Unpin,
    {
        let address = match host.parse::<IpAddr>() {
            Ok(ip) => Address::Socket(SocketAddr::new(ip, port)),
            Err(_) if host.len() <= 255 => {
                if self.local_dns {
                    let socket = (host, port)
                        .to_socket_addrs()?
                        .next()
                        .ok_or(SocksError::DnsFailure)?;
                    Address::Socket(socket)
                } else {
                    Address::Domain(host, port)
                }
            }
            Err(_) => return Err(SocksV5Error::HostTooLong.into()),
        };
        let method = if self.proxy_auth.is_some() {
            AuthMethod::UserPass
        } else {
            AuthMethod::NoAuth
        };
        let mut recv_buf = BytesMut::with_capacity(513);
        let mut send_buf = BytesMut::with_capacity(262);
        let mut state = State::SendingNegReq;
        loop {
            match state {
                State::SendingNegReq => {
                    let req = NegotiationReq(&method);
                    let start = send_buf.len();
                    req.write_to_buf(&mut send_buf)?;
                    crate::hyper_util::rt::write_all(&mut conn, &send_buf[start..]).await?;
                    if self.optimistic {
                        if method == AuthMethod::UserPass {
                            state = State::SendingAuthReq;
                        } else {
                            state = State::SendingProxyReq;
                        }
                    } else {
                        state = State::ReadingNegRes;
                    }
                }
                State::ReadingNegRes => {
                    let res: NegotiationRes = super::read_message(&mut conn, &mut recv_buf).await?;
                    if res.0 == AuthMethod::NoneAcceptable {
                        return Err(SocksV5Error::Auth(AuthError::Unsupported).into());
                    }
                    if res.0 != method {
                        return Err(SocksV5Error::Auth(AuthError::MethodMismatch).into());
                    }
                    if self.optimistic {
                        if res.0 == AuthMethod::UserPass {
                            state = State::ReadingAuthRes;
                        } else {
                            state = State::ReadingProxyRes;
                        }
                    } else if res.0 == AuthMethod::UserPass {
                        state = State::SendingAuthReq;
                    } else {
                        state = State::SendingProxyReq;
                    }
                }
                State::SendingAuthReq => {
                    let (user, pass) = self.proxy_auth.as_ref().unwrap();
                    let req = AuthenticationReq(user, pass);
                    let start = send_buf.len();
                    req.write_to_buf(&mut send_buf)?;
                    crate::hyper_util::rt::write_all(&mut conn, &send_buf[start..]).await?;
                    if self.optimistic {
                        state = State::SendingProxyReq;
                    } else {
                        state = State::ReadingAuthRes;
                    }
                }
                State::ReadingAuthRes => {
                    let res: AuthenticationRes =
                        super::read_message(&mut conn, &mut recv_buf).await?;
                    if !res.0 {
                        return Err(SocksV5Error::Auth(AuthError::Failed).into());
                    }
                    if self.optimistic {
                        state = State::ReadingProxyRes;
                    } else {
                        state = State::SendingProxyReq;
                    }
                }
                State::SendingProxyReq => {
                    let req = ProxyReq(&address);
                    let start = send_buf.len();
                    req.write_to_buf(&mut send_buf)?;
                    crate::hyper_util::rt::write_all(&mut conn, &send_buf[start..]).await?;
                    if self.optimistic {
                        state = State::ReadingNegRes;
                    } else {
                        state = State::ReadingProxyRes;
                    }
                }
                State::ReadingProxyRes => {
                    let res: ProxyRes = super::read_message(&mut conn, &mut recv_buf).await?;
                    if res.0 == Status::Success {
                        return Ok(conn);
                    } else {
                        return Err(SocksV5Error::Command(res.0).into());
                    }
                }
            }
        }
    }
}
impl<C> Service<Uri> for SocksV5<C>
where
    C: Service<Uri>,
    C::Future: Send + 'static,
    C::Response: Read + Write + Unpin + Send + 'static,
    C::Error: Send + 'static,
{
    type Response = C::Response;
    type Error = SocksError<C::Error>;
    type Future = Handshaking<C::Future, C::Response, C::Error>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(SocksError::Inner)
    }
    fn call(&mut self, dst: Uri) -> Self::Future {
        let config = self.config.clone();
        let connecting = self.inner.call(config.proxy.clone());
        let fut = async move {
            let port = dst.port().map(|p| p.as_u16()).unwrap_or(443);
            let host_from_uri = dst.host().ok_or(SocksError::MissingHost)?;
            let host = crate::hyper_util::client::strip_ipv6_brackets(host_from_uri).to_string();
            let conn = connecting.await.map_err(SocksError::Inner)?;
            config.execute(conn, host, port).await
        };
        Handshaking {
            fut: Box::pin(fut),
            _marker: Default::default(),
        }
    }
}

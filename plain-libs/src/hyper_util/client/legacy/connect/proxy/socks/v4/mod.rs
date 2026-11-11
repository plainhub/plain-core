mod errors;
pub use errors::*;
mod messages;
use super::{Handshaking, SocksError};
use crate::http::Uri;
use crate::hyper::rt::{Read, Write};
use bytes::BytesMut;
use messages::*;
use std::net::{IpAddr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::task::{Context, Poll};
use tower_service::Service;
#[derive(Debug, Clone)]
pub struct SocksV4<C> {
    inner: C,
    config: SocksConfig,
}
#[derive(Debug, Clone)]
struct SocksConfig {
    proxy: Uri,
    local_dns: bool,
}
impl<C> SocksV4<C> {
    pub fn new(proxy_dst: Uri, connector: C) -> Self {
        Self {
            inner: connector,
            config: SocksConfig::new(proxy_dst),
        }
    }
    pub fn local_dns(mut self, local_dns: bool) -> Self {
        self.config.local_dns = local_dns;
        self
    }
}
impl SocksConfig {
    pub fn new(proxy: Uri) -> Self {
        Self {
            proxy,
            local_dns: false,
        }
    }
    async fn execute<T, E>(self, mut conn: T, host: String, port: u16) -> Result<T, SocksError<E>>
    where
        T: Read + Write + Unpin,
    {
        let address = match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => return Err(SocksV4Error::IpV6.into()),
            Ok(IpAddr::V4(ip)) => Address::Socket(SocketAddrV4::new(ip, port)),
            Err(_) => {
                if self.local_dns {
                    (host, port)
                        .to_socket_addrs()?
                        .find_map(|s| {
                            if let SocketAddr::V4(v4) = s {
                                Some(Address::Socket(v4))
                            } else {
                                None
                            }
                        })
                        .ok_or(SocksError::DnsFailure)?
                } else {
                    Address::Domain(host, port)
                }
            }
        };
        let mut send_buf = BytesMut::with_capacity(1024);
        let mut recv_buf = BytesMut::with_capacity(1024);
        let req = Request(&address);
        let n = req.write_to_buf(&mut send_buf)?;
        crate::hyper_util::rt::write_all(&mut conn, &send_buf[..n]).await?;
        let res: Response = super::read_message(&mut conn, &mut recv_buf).await?;
        if res.0 == Status::Success {
            Ok(conn)
        } else {
            Err(SocksV4Error::Command(res.0).into())
        }
    }
}
impl<C> Service<Uri> for SocksV4<C>
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

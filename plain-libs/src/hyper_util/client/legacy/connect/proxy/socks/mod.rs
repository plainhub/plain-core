mod v5;
pub use v5::{SocksV5, SocksV5Error};
mod v4;
use crate::hyper::rt::Read;
use bytes::{Buf, BytesMut};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
pub use v4::{SocksV4, SocksV4Error};
#[derive(Debug)]
pub enum SocksError<C> {
    Inner(C),
    Io(std::io::Error),
    DnsFailure,
    MissingHost,
    MissingPort,
    V4(SocksV4Error),
    V5(SocksV5Error),
    Parsing(ParsingError),
    Serialize(SerializeError),
}
#[derive(Debug)]
pub enum ParsingError {
    Incomplete,
    WouldOverflow,
    Other,
}
#[derive(Debug)]
pub enum SerializeError {
    WouldOverflow,
}
async fn read_message<T, M, C>(mut conn: &mut T, buf: &mut BytesMut) -> Result<M, SocksError<C>>
where
    T: Read + Unpin,
    M: for<'a, 'b> TryFrom<&'a mut &'b [u8], Error = ParsingError>,
{
    let mut tmp = [0; 513];
    loop {
        let mut view = &buf[..];
        match M::try_from(&mut view) {
            Err(ParsingError::Incomplete) => {
                let n = crate::hyper_util::rt::read(&mut conn, &mut tmp).await?;
                if n == 0 {
                    if buf.spare_capacity_mut().is_empty() {
                        return Err(SocksError::Parsing(ParsingError::WouldOverflow));
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "unexpected eof",
                        )
                        .into());
                    }
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(err) => return Err(err.into()),
            Ok(res) => {
                let consumed = buf.len() - view.len();
                buf.advance(consumed);
                return Ok(res);
            }
        }
    }
}
impl<C> std::fmt::Display for SocksError<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SOCKS error: ")?;
        match self {
            Self::Inner(_) => f.write_str("failed to create underlying connection"),
            Self::Io(_) => f.write_str("io error during SOCKS handshake"),
            Self::DnsFailure => f.write_str("could not resolve to acceptable address type"),
            Self::MissingHost => f.write_str("missing destination host"),
            Self::MissingPort => f.write_str("missing destination port"),
            Self::Parsing(_) => f.write_str("failed parsing server response"),
            Self::Serialize(_) => f.write_str("failed serialize request"),
            Self::V4(e) => e.fmt(f),
            Self::V5(e) => e.fmt(f),
        }
    }
}
impl<C: std::fmt::Debug + std::fmt::Display> std::error::Error for SocksError<C> {}
impl<C> From<std::io::Error> for SocksError<C> {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
impl<C> From<ParsingError> for SocksError<C> {
    fn from(err: ParsingError) -> Self {
        Self::Parsing(err)
    }
}
impl<C> From<SerializeError> for SocksError<C> {
    fn from(err: SerializeError) -> Self {
        Self::Serialize(err)
    }
}
impl<C> From<SocksV4Error> for SocksError<C> {
    fn from(err: SocksV4Error) -> Self {
        Self::V4(err)
    }
}
impl<C> From<SocksV5Error> for SocksError<C> {
    fn from(err: SocksV5Error) -> Self {
        Self::V5(err)
    }
}
pin_project! { #[must_use = "futures do nothing unless polled"] pub struct Handshaking < F , T , E > { #[pin] fut : BoxHandshaking < T , E >, _marker : std::marker::PhantomData < F > } }
type BoxHandshaking<T, E> = Pin<Box<dyn Future<Output = Result<T, SocksError<E>>> + Send>>;
impl<F, T, E> Future for Handshaking<F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, SocksError<E>>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}

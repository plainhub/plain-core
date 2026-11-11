use crate::axum_core::{BoxError, Error};
use crate::http_body::{Body as _, Frame};
use crate::http_body_util::BodyExt;
use crate::sync_wrapper::SyncWrapper;
use bytes::Bytes;
use futures_util::TryStream;
use futures_util::stream::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
type BoxBody = crate::http_body_util::combinators::UnsyncBoxBody<Bytes, Error>;
fn boxed<B>(body: B) -> BoxBody
where
    B: crate::http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    try_downcast(body).unwrap_or_else(|body| body.map_err(Error::new).boxed_unsync())
}
pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}
#[derive(Debug)]
pub struct Body(BoxBody);
impl Body {
    pub fn new<B>(body: B) -> Self
    where
        B: crate::http_body::Body<Data = Bytes> + Send + 'static,
        B::Error: Into<BoxError>,
    {
        try_downcast(body).unwrap_or_else(|body| Self(boxed(body)))
    }
    pub fn empty() -> Self {
        Self::new(crate::http_body_util::Empty::new())
    }
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: TryStream + Send + 'static,
        S::Ok: Into<Bytes>,
        S::Error: Into<BoxError>,
    {
        Self::new(StreamBody {
            stream: SyncWrapper::new(stream),
        })
    }
    pub fn into_data_stream(self) -> BodyDataStream {
        BodyDataStream { inner: self }
    }
}
impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}
impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}
macro_rules! body_from_impl {
    ($ty : ty) => {
        impl From<$ty> for Body {
            fn from(buf: $ty) -> Self {
                Self::new(crate::http_body_util::Full::from(buf))
            }
        }
    };
}
body_from_impl!(&'static [u8]);
body_from_impl!(std::borrow::Cow<'static, [u8]>);
body_from_impl!(Vec<u8>);
body_from_impl!(&'static str);
body_from_impl!(std::borrow::Cow<'static, str>);
body_from_impl!(String);
body_from_impl!(Bytes);
impl crate::http_body::Body for Body {
    type Data = Bytes;
    type Error = Error;
    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }
    #[inline]
    fn size_hint(&self) -> crate::http_body::SizeHint {
        self.0.size_hint()
    }
    #[inline]
    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }
}
#[derive(Debug)]
pub struct BodyDataStream {
    inner: Body,
}
impl Stream for BodyDataStream {
    type Item = Result<Bytes, Error>;
    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match futures_util::ready!(Pin::new(&mut self.inner).poll_frame(cx)?) {
                Some(frame) => match frame.into_data() {
                    Ok(data) => return Poll::Ready(Some(Ok(data))),
                    Err(_frame) => {}
                },
                None => return Poll::Ready(None),
            }
        }
    }
}
impl crate::http_body::Body for BodyDataStream {
    type Data = Bytes;
    type Error = Error;
    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }
    #[inline]
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    #[inline]
    fn size_hint(&self) -> crate::http_body::SizeHint {
        self.inner.size_hint()
    }
}
pin_project! { struct StreamBody < S > { #[pin] stream : SyncWrapper < S >, } }
impl<S> crate::http_body::Body for StreamBody<S>
where
    S: TryStream,
    S::Ok: Into<Bytes>,
    S::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let stream = self.project().stream.get_pin_mut();
        match futures_util::ready!(stream.try_poll_next(cx)) {
            Some(Ok(chunk)) => Poll::Ready(Some(Ok(Frame::data(chunk.into())))),
            Some(Err(err)) => Poll::Ready(Some(Err(Error::new(err)))),
            None => Poll::Ready(None),
        }
    }
}

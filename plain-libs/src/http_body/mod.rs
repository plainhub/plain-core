mod frame;
mod size_hint;
pub use self::frame::Frame;
pub use self::size_hint::SizeHint;
use bytes::{Buf, Bytes};
use std::convert::Infallible;
use std::ops;
use std::pin::Pin;
use std::task::{Context, Poll};
pub trait Body {
    type Data: Buf;
    type Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>>;
    fn is_end_stream(&self) -> bool {
        false
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
impl<T: Body + Unpin + ?Sized> Body for &mut T {
    type Data = T::Data;
    type Error = T::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut **self).poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        Pin::new(&**self).is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        Pin::new(&**self).size_hint()
    }
}
impl<P> Body for Pin<P>
where
    P: Unpin + ops::DerefMut,
    P::Target: Body,
{
    type Data = <<P as ops::Deref>::Target as Body>::Data;
    type Error = <<P as ops::Deref>::Target as Body>::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::get_mut(self).as_mut().poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        self.as_ref().is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.as_ref().size_hint()
    }
}
impl<T: Body + Unpin + ?Sized> Body for Box<T> {
    type Data = T::Data;
    type Error = T::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut **self).poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        self.as_ref().is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.as_ref().size_hint()
    }
}
impl<B: Body> Body for crate::http::Request<B> {
    type Data = B::Data;
    type Error = B::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        unsafe {
            self.map_unchecked_mut(crate::http::Request::body_mut)
                .poll_frame(cx)
        }
    }
    fn is_end_stream(&self) -> bool {
        self.body().is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.body().size_hint()
    }
}
impl<B: Body> Body for crate::http::Response<B> {
    type Data = B::Data;
    type Error = B::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        unsafe {
            self.map_unchecked_mut(crate::http::Response::body_mut)
                .poll_frame(cx)
        }
    }
    fn is_end_stream(&self) -> bool {
        self.body().is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.body().size_hint()
    }
}
impl Body for String {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if !self.is_empty() {
            let s = std::mem::take(&mut *self);
            Poll::Ready(Some(Ok(Frame::data(s.into_bytes().into()))))
        } else {
            Poll::Ready(None)
        }
    }
    fn is_end_stream(&self) -> bool {
        self.is_empty()
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.len() as u64)
    }
}

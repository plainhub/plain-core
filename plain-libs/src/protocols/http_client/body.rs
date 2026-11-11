use super::{Error, error::BoxError};
use bytes::Bytes;
use futures_core::Stream;
use crate::http_body::{Body as HttpBody, Frame, SizeHint};
use crate::http_body_util::{
    BodyDataStream, BodyExt, Either, Full, StreamBody, combinators::UnsyncBoxBody,
};
use crate::hyper::body::Incoming;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};
use tokio::time::{Instant, Sleep};

pub struct Body(Either<Full<Bytes>, UnsyncBoxBody<Bytes, BoxError>>);

impl Body {
    pub fn wrap_stream<S, E>(stream: S) -> Self
    where
        S: Stream<Item = Result<Bytes, E>> + Send + 'static,
        E: Into<BoxError>,
    {
        Self(Either::Right(
            StreamBody::new(DataFrames(stream)).boxed_unsync(),
        ))
    }

    pub(super) fn try_clone(&self) -> Option<Self> {
        match &self.0 {
            Either::Left(body) => Some(Self(Either::Left(body.clone()))),
            Either::Right(_) => None,
        }
    }
}

struct DataFrames<S>(S);

impl<S, E> Stream for DataFrames<S>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    type Item = Result<Frame<Bytes>, BoxError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // SAFETY: The stream stays pinned inside its wrapper.
        unsafe { self.map_unchecked_mut(|this| &mut this.0) }
            .poll_next(cx)
            .map(|item| item.map(|item| item.map(Frame::data).map_err(Into::into)))
    }
}

impl<T: Into<Bytes>> From<T> for Body {
    fn from(value: T) -> Self {
        Self(Either::Left(Full::new(value.into())))
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::from(Bytes::new())
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.0.size_hint()
    }
}

pub(super) struct ResponseBody {
    body: Incoming,
    total: Option<Pin<Box<Sleep>>>,
    read: Option<ReadDeadline>,
}

struct ReadDeadline {
    duration: Duration,
    timer: Pin<Box<Sleep>>,
    waiting: bool,
}

impl ResponseBody {
    pub(super) fn new(
        body: Incoming,
        total: Option<Pin<Box<Sleep>>>,
        read: Option<Duration>,
    ) -> Self {
        Self {
            body,
            total,
            read: read.map(|duration| ReadDeadline {
                duration,
                timer: Box::pin(tokio::time::sleep(duration)),
                waiting: false,
            }),
        }
    }

    pub(super) fn bytes_stream(self) -> impl Stream<Item = Result<Bytes, Error>> {
        BodyDataStream::new(self)
    }
}

impl HttpBody for ResponseBody {
    type Data = Bytes;
    type Error = Error;

    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        if let Some(total) = self.total.as_mut()
            && total.as_mut().poll(cx).is_ready()
        {
            return Poll::Ready(Some(Err(Error::decode(timeout()))));
        }

        if let Some(read) = self.read.as_mut() {
            if !read.waiting {
                read.timer.as_mut().reset(Instant::now() + read.duration);
                read.waiting = true;
            }
            if read.timer.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Some(Err(Error::decode(timeout()))));
            }
        }

        let frame = ready!(Pin::new(&mut self.body).poll_frame(cx));
        if let Some(read) = self.read.as_mut() {
            read.waiting = false;
        }
        Poll::Ready(frame.map(|result| result.map_err(Error::decode)))
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

pub(super) fn timeout() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out")
}

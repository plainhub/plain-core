use crate::http::HeaderMap;
use crate::http_body::{Body, Frame};
use crate::http_body_util::util::BufList;
use bytes::{Buf, Bytes};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};
#[derive(Debug)]
pub struct Collected<B> {
    bufs: BufList<B>,
    trailers: Option<HeaderMap>,
}
impl<B: Buf> Collected<B> {
    pub fn trailers(&self) -> Option<&HeaderMap> {
        self.trailers.as_ref()
    }
    pub fn aggregate(self) -> impl Buf {
        self.bufs
    }
    pub fn to_bytes(mut self) -> Bytes {
        self.bufs.copy_to_bytes(self.bufs.remaining())
    }
    pub(crate) fn push_frame(&mut self, frame: Frame<B>) {
        let frame = match frame.into_data() {
            Ok(data) => {
                if data.has_remaining() {
                    self.bufs.push(data);
                }
                return;
            }
            Err(frame) => frame,
        };
        if let Ok(trailers) = frame.into_trailers() {
            if let Some(current) = &mut self.trailers {
                current.extend(trailers);
            } else {
                self.trailers = Some(trailers);
            }
        };
    }
}
impl<B: Buf> Body for Collected<B> {
    type Data = B;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = if let Some(data) = self.bufs.pop() {
            Frame::data(data)
        } else if let Some(trailers) = self.trailers.take() {
            Frame::trailers(trailers)
        } else {
            return Poll::Ready(None);
        };
        Poll::Ready(Some(Ok(frame)))
    }
}
impl<B> Default for Collected<B> {
    fn default() -> Self {
        Self {
            bufs: BufList::default(),
            trailers: None,
        }
    }
}
impl<B> Unpin for Collected<B> {}

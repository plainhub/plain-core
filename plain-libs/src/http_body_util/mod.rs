mod collected;
pub mod combinators;
mod either;
mod empty;
mod full;
mod limited;
mod stream;
mod util;
pub use self::collected::Collected;
use self::combinators::{BoxBody, MapErr, MapFrame, UnsyncBoxBody};
pub use self::either::Either;
pub use self::empty::Empty;
pub use self::full::Full;
pub use self::limited::{LengthLimitError, Limited};
pub use self::stream::{BodyDataStream, BodyStream, StreamBody};
pub trait BodyExt: crate::http_body::Body {
    fn frame(&mut self) -> combinators::Frame<'_, Self>
    where
        Self: Unpin,
    {
        combinators::Frame(self)
    }
    fn map_frame<F, B>(self, f: F) -> MapFrame<Self, F>
    where
        Self: Sized,
        F: FnMut(crate::http_body::Frame<Self::Data>) -> crate::http_body::Frame<B>,
        B: bytes::Buf,
    {
        MapFrame::new(self, f)
    }
    fn inspect_frame<F>(self, f: F) -> combinators::InspectFrame<Self, F>
    where
        Self: Sized,
        F: FnMut(&crate::http_body::Frame<Self::Data>),
    {
        combinators::InspectFrame::new(self, f)
    }
    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Error) -> E,
    {
        MapErr::new(self, f)
    }
    fn inspect_err<F>(self, f: F) -> combinators::InspectErr<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Error),
    {
        combinators::InspectErr::new(self, f)
    }
    fn boxed(self) -> BoxBody<Self::Data, Self::Error>
    where
        Self: Sized + Send + Sync + 'static,
    {
        BoxBody::new(self)
    }
    fn boxed_unsync(self) -> UnsyncBoxBody<Self::Data, Self::Error>
    where
        Self: Sized + Send + 'static,
    {
        UnsyncBoxBody::new(self)
    }
    fn collect(self) -> combinators::Collect<Self>
    where
        Self: Sized,
    {
        combinators::Collect {
            body: self,
            collected: Some(crate::http_body_util::Collected::default()),
        }
    }
    fn with_trailers<F>(self, trailers: F) -> combinators::WithTrailers<Self, F>
    where
        Self: Sized,
        F: std::future::Future<Output = Option<Result<crate::http::HeaderMap, Self::Error>>>,
    {
        combinators::WithTrailers::new(self, trailers)
    }
    fn into_stream(self) -> BodyStream<Self>
    where
        Self: Sized,
    {
        BodyStream::new(self)
    }
    fn into_data_stream(self) -> BodyDataStream<Self>
    where
        Self: Sized,
    {
        BodyDataStream::new(self)
    }
    fn fuse(self) -> combinators::Fuse<Self>
    where
        Self: Sized,
    {
        combinators::Fuse::new(self)
    }
}
impl<T: ?Sized> BodyExt for T where T: crate::http_body::Body {}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/http_body_util.rs"]
mod tests;

use crate::http_body::{Body, Frame, SizeHint};
use bytes::Buf;
use proj::EitherProj;
use std::error::Error;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};
#[derive(Debug, Clone, Copy)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}
impl<L, R> Either<L, R> {
    pub(crate) fn project(self: Pin<&mut Self>) -> EitherProj<'_, L, R> {
        unsafe {
            match self.get_unchecked_mut() {
                Self::Left(left) => EitherProj::Left(Pin::new_unchecked(left)),
                Self::Right(right) => EitherProj::Right(Pin::new_unchecked(right)),
            }
        }
    }
}
impl<L> Either<L, L> {
    pub fn into_inner(self) -> L {
        match self {
            Either::Left(left) => left,
            Either::Right(right) => right,
        }
    }
}
impl<L, R, Data> Body for Either<L, R>
where
    L: Body<Data = Data>,
    R: Body<Data = Data>,
    L::Error: Into<Box<dyn Error + Send + Sync>>,
    R::Error: Into<Box<dyn Error + Send + Sync>>,
    Data: Buf,
{
    type Data = Data;
    type Error = Box<dyn Error + Send + Sync>;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project() {
            EitherProj::Left(left) => left
                .poll_frame(cx)
                .map(|poll| poll.map(|opt| opt.map_err(Into::into))),
            EitherProj::Right(right) => right
                .poll_frame(cx)
                .map(|poll| poll.map(|opt| opt.map_err(Into::into))),
        }
    }
    fn is_end_stream(&self) -> bool {
        match self {
            Either::Left(left) => left.is_end_stream(),
            Either::Right(right) => right.is_end_stream(),
        }
    }
    fn size_hint(&self) -> SizeHint {
        match self {
            Either::Left(left) => left.size_hint(),
            Either::Right(right) => right.size_hint(),
        }
    }
}
pub(crate) mod proj {
    use super::Either;
    use std::marker::PhantomData;
    use std::pin::Pin;
    pub(crate) enum EitherProj<'__pin, L, R>
    where
        Either<L, R>: '__pin,
    {
        Left(Pin<&'__pin mut L>),
        Right(Pin<&'__pin mut R>),
    }
    const _: () = {
        pub struct __Origin<'__pin, L, R> {
            __dummy_lifetime: PhantomData<&'__pin ()>,
            _Left: L,
            _Right: R,
        }
        impl<'__pin, L, R> Unpin for Either<L, R> where __Origin<'__pin, L, R>: Unpin {}
        trait MustNotImplDrop {}
        impl<T: Drop> MustNotImplDrop for T {}
        impl<L, R> MustNotImplDrop for Either<L, R> {}
    };
}

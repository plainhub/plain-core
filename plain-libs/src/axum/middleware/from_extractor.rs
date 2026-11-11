use crate::axum::{
    extract::FromRequestParts,
    response::{IntoResponse, Response},
};
use crate::http::Request;
use futures_util::{future::BoxFuture, ready};
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;
pub fn from_extractor<E>() -> FromExtractorLayer<E, ()> {
    from_extractor_with_state(())
}
pub fn from_extractor_with_state<E, S>(state: S) -> FromExtractorLayer<E, S> {
    FromExtractorLayer {
        state,
        _marker: PhantomData,
    }
}
#[must_use]
pub struct FromExtractorLayer<E, S> {
    state: S,
    _marker: PhantomData<fn() -> E>,
}
impl<E, S> Clone for FromExtractorLayer<E, S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}
impl<E, S> fmt::Debug for FromExtractorLayer<E, S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromExtractorLayer")
            .field("state", &self.state)
            .field("extractor", &format_args!("{}", std::any::type_name::<E>()))
            .finish()
    }
}
impl<E, T, S> Layer<T> for FromExtractorLayer<E, S>
where
    S: Clone,
{
    type Service = FromExtractor<T, E, S>;
    fn layer(&self, inner: T) -> Self::Service {
        FromExtractor {
            inner,
            state: self.state.clone(),
            _extractor: PhantomData,
        }
    }
}
pub struct FromExtractor<T, E, S> {
    inner: T,
    state: S,
    _extractor: PhantomData<fn() -> E>,
}
impl<T, E, S> Clone for FromExtractor<T, E, S>
where
    T: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: self.state.clone(),
            _extractor: PhantomData,
        }
    }
}
impl<T, E, S> fmt::Debug for FromExtractor<T, E, S>
where
    T: fmt::Debug,
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromExtractor")
            .field("inner", &self.inner)
            .field("state", &self.state)
            .field("extractor", &format_args!("{}", std::any::type_name::<E>()))
            .finish()
    }
}
impl<T, E, B, S> Service<Request<B>> for FromExtractor<T, E, S>
where
    E: FromRequestParts<S> + 'static,
    B: Send + 'static,
    T: Service<Request<B>> + Clone,
    T::Response: IntoResponse,
    S: Clone + Send + Sync + 'static,
{
    type Response = Response;
    type Error = T::Error;
    type Future = ResponseFuture<B, T, E, S>;
    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        let state = self.state.clone();
        let extract_future = Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let extracted = E::from_request_parts(&mut parts, &state).await;
            let req = Request::from_parts(parts, body);
            (req, extracted)
        });
        ResponseFuture {
            state: State::Extracting {
                future: extract_future,
            },
            svc: Some(self.inner.clone()),
        }
    }
}
pin_project! { pub struct ResponseFuture < B , T , E , S > where E : FromRequestParts < S >, T : Service < Request < B >>, { #[pin] state : State < B , T , E , S >, svc : Option < T >, } }
pin_project! { #[project = StateProj] enum State < B , T , E , S > where E : FromRequestParts < S >, T : Service < Request < B >>, { Extracting { future : BoxFuture <'static , (Request < B >, Result < E , E::Rejection >) >, } , Call { #[pin] future : T::Future } , } }
impl<B, T, E, S> Future for ResponseFuture<B, T, E, S>
where
    E: FromRequestParts<S>,
    T: Service<Request<B>>,
    T::Response: IntoResponse,
{
    type Output = Result<Response, T::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            let new_state = match this.state.as_mut().project() {
                StateProj::Extracting { future } => {
                    let (req, extracted) = ready!(future.as_mut().poll(cx));
                    match extracted {
                        Ok(_) => {
                            let mut svc = this.svc.take().expect("future polled after completion");
                            let future = svc.call(req);
                            State::Call { future }
                        }
                        Err(err) => {
                            let res = err.into_response();
                            return Poll::Ready(Ok(res));
                        }
                    }
                }
                StateProj::Call { future } => {
                    return future
                        .poll(cx)
                        .map(|result| result.map(IntoResponse::into_response));
                }
            };
            this.state.set(new_state);
        }
    }
}

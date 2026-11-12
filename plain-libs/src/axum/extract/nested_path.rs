use super::rejection::NestedPathRejection;
use crate::axum::extract::Request;
use crate::axum_core::extract::FromRequestParts;
use crate::http::request::Parts;
use std::{
    sync::Arc,
    task::{Context, Poll},
};
use crate::tower::layer::{Layer, layer_fn};
use crate::tower::Service;
#[derive(Debug, Clone)]
pub struct NestedPath(Arc<str>);
impl NestedPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> FromRequestParts<S> for NestedPath
where
    S: Send + Sync,
{
    type Rejection = NestedPathRejection;
    async fn from_request_parts<'extract>(parts: &'extract mut Parts, _state: &'extract S) -> Result<Self, Self::Rejection>  where Self: 'extract {
        match parts.extensions.get::<Self>() {
            Some(nested_path) => Ok(nested_path.clone()),
            None => Err(NestedPathRejection),
        }
    }
}
#[derive(Clone)]
pub(crate) struct SetNestedPath<S> {
    inner: S,
    path: Arc<str>,
}
impl<S> SetNestedPath<S> {
    pub(crate) fn layer(path: &str) -> impl Layer<S, Service = Self> + Clone + use<S> {
        let path = Arc::from(path);
        layer_fn(move |inner| Self {
            inner,
            path: Arc::clone(&path),
        })
    }
}
impl<S, B> Service<Request<B>> for SetNestedPath<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(prev) = req.extensions_mut().get_mut::<NestedPath>() {
            let new_path = if prev.as_str() == "/" {
                Arc::clone(&self.path)
            } else {
                format!("{}{}", prev.as_str().trim_end_matches('/'), self.path).into()
            };
            prev.0 = new_path;
        } else {
            req.extensions_mut()
                .insert(NestedPath(Arc::clone(&self.path)));
        };
        self.inner.call(req)
    }
}

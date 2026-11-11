use crate::http::{Request, Uri};
use crate::tower::Layer;
use std::{
    sync::Arc,
    task::{Context, Poll},
};
use tower_layer::layer_fn;
use tower_service::Service;
#[derive(Clone)]
pub(super) struct StripPrefix<S> {
    inner: S,
    prefix: Arc<str>,
}
impl<S> StripPrefix<S> {
    pub(super) fn layer(prefix: &str) -> impl Layer<S, Service = Self> + Clone + use<S> {
        let prefix = Arc::from(prefix);
        layer_fn(move |inner| Self {
            inner,
            prefix: Arc::clone(&prefix),
        })
    }
}
impl<S, B> Service<Request<B>> for StripPrefix<S>
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
        if let Some(new_uri) = strip_prefix(req.uri(), &self.prefix) {
            *req.uri_mut() = new_uri;
        }
        self.inner.call(req)
    }
}
fn strip_prefix(uri: &Uri, prefix: &str) -> Option<Uri> {
    let path_and_query = uri.path_and_query()?;
    let mut matching_prefix_length = Some(0);
    for item in zip_longest(segments(path_and_query.path()), segments(prefix)) {
        *matching_prefix_length.as_mut().unwrap() += 1;
        match item {
            Item::Both(path_segment, prefix_segment) => {
                if prefix_segment.starts_with(':') || path_segment == prefix_segment {
                    *matching_prefix_length.as_mut().unwrap() += path_segment.len();
                } else if prefix_segment.is_empty() {
                    break;
                } else {
                    matching_prefix_length = None;
                    break;
                }
            }
            Item::First(_) => {
                break;
            }
            Item::Second(_) => {
                matching_prefix_length = None;
                break;
            }
        }
    }
    let after_prefix = uri.path().split_at(matching_prefix_length?).1;
    let new_path_and_query = match (after_prefix.starts_with('/'), path_and_query.query()) {
        (true, None) => after_prefix.parse().unwrap(),
        (true, Some(query)) => format!("{after_prefix}?{query}").parse().unwrap(),
        (false, None) => format!("/{after_prefix}").parse().unwrap(),
        (false, Some(query)) => format!("/{after_prefix}?{query}").parse().unwrap(),
    };
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(new_path_and_query);
    Some(Uri::from_parts(parts).unwrap())
}
fn segments(s: &str) -> impl Iterator<Item = &str> {
    assert!(
        s.starts_with('/'),
        "path didn't start with '/'. axum should have caught this higher up."
    );
    s.split('/').skip(1)
}
fn zip_longest<I, I2>(a: I, b: I2) -> impl Iterator<Item = Item<I::Item>>
where
    I: Iterator,
    I2: Iterator<Item = I::Item>,
{
    let a = a.map(Some).chain(std::iter::repeat_with(|| None));
    let b = b.map(Some).chain(std::iter::repeat_with(|| None));
    a.zip(b).map_while(|(a, b)| match (a, b) {
        (Some(a), Some(b)) => Some(Item::Both(a, b)),
        (Some(a), None) => Some(Item::First(a)),
        (None, Some(b)) => Some(Item::Second(b)),
        (None, None) => None,
    })
}
#[derive(Debug)]
enum Item<T> {
    Both(T, T),
    First(T),
    Second(T),
}

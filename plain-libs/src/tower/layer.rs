use std::fmt;

// Adapted from tower-layer0.3.3; see SOURCE.json and LICENSE.
pub trait Layer<S> {
    type Service;
    fn layer(&self, inner: S) -> Self::Service;
}

pub fn layer_fn<F>(f: F) -> LayerFn<F> {
    LayerFn { f }
}

#[derive(Clone, Copy)]
pub struct LayerFn<F> {
    f: F,
}

impl<F, S, Out> Layer<S> for LayerFn<F>
where
    F: Fn(S) -> Out,
{
    type Service = Out;
    fn layer(&self, inner: S) -> Self::Service {
        (self.f)(inner)
    }
}

impl<F> fmt::Debug for LayerFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayerFn")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tower/layer.rs"]
mod tests;

impl<S, A, B> Layer<S> for (A, B)
where
    B: Layer<S>,
    A: Layer<B::Service>,
{
    type Service = A::Service;
    fn layer(&self, service: S) -> Self::Service {
        self.0.layer(self.1.layer(service))
    }
}

impl<S, A, B, C> Layer<S> for (A, B, C)
where
    C: Layer<S>,
    B: Layer<C::Service>,
    A: Layer<B::Service>,
{
    type Service = A::Service;
    fn layer(&self, service: S) -> Self::Service {
        self.0.layer(self.1.layer(self.2.layer(service)))
    }
}

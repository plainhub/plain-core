use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct OnInformational(Arc<dyn OnInformationalCallback + Send + Sync>);
pub fn on_informational<B, F>(req: &mut crate::http::Request<B>, callback: F)
where
    F: Fn(Response<'_>) + Send + Sync + 'static,
{
    on_informational_raw(req, OnInformationalClosure(callback));
}
pub(crate) fn on_informational_raw<B, C>(req: &mut crate::http::Request<B>, callback: C)
where
    C: OnInformationalCallback + Send + Sync + 'static,
{
    req.extensions_mut()
        .insert(OnInformational(Arc::new(callback)));
}
pub(crate) trait OnInformationalCallback {
    fn on_informational(&self, res: crate::http::Response<()>);
}
impl OnInformational {
    pub(crate) fn call(&self, res: crate::http::Response<()>) {
        self.0.on_informational(res);
    }
}
struct OnInformationalClosure<F>(F);
impl<F> OnInformationalCallback for OnInformationalClosure<F>
where
    F: Fn(Response<'_>) + Send + Sync + 'static,
{
    fn on_informational(&self, res: crate::http::Response<()>) {
        let res = Response(&res);
        (self.0)(res);
    }
}
#[derive(Debug)]
pub struct Response<'a>(&'a crate::http::Response<()>);
impl Response<'_> {
    #[inline]
    pub fn status(&self) -> crate::http::StatusCode {
        self.0.status()
    }
    #[inline]
    pub fn version(&self) -> crate::http::Version {
        self.0.version()
    }
    #[inline]
    pub fn headers(&self) -> &crate::http::HeaderMap {
        self.0.headers()
    }
}

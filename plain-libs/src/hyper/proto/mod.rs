cfg_feature! { #! [all ()] pub (crate) mod h1 ; pub (crate) use self::h1::Conn ; pub (crate) use self::h1::dispatch ; pub (crate) use self::h1::ServerTransaction ; }
pub(crate) mod h2;
#[derive(Debug, Default)]
pub(crate) struct MessageHead<S> {
    pub(crate) version: crate::http::Version,
    pub(crate) subject: S,
    pub(crate) headers: crate::http::HeaderMap,
    extensions: crate::http::Extensions,
}
pub(crate) type RequestHead = MessageHead<RequestLine>;
#[derive(Debug, Default, PartialEq)]
pub(crate) struct RequestLine(pub(crate) crate::http::Method, pub(crate) crate::http::Uri);
pub(crate) type ResponseHead = MessageHead<crate::http::StatusCode>;
#[derive(Debug)]
pub(crate) enum BodyLength {
    Known(u64),
    Unknown,
}
pub(crate) enum Dispatched {
    Shutdown,
    Upgrade(crate::hyper::upgrade::Pending),
}
impl MessageHead<crate::http::StatusCode> {
    fn into_response<B>(self, body: B) -> crate::http::Response<B> {
        let mut res = crate::http::Response::new(body);
        *res.status_mut() = self.subject;
        *res.headers_mut() = self.headers;
        *res.version_mut() = self.version;
        *res.extensions_mut() = self.extensions;
        res
    }
}

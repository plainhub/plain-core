// Native request behavior adapted from reqwest 0.12.28 (https://github.com/seanmonstar/reqwest), see LICENSE-MIT.

mod body;
mod connector;
mod error;
pub mod redirect;
mod tls;

pub use body::Body;
pub use error::Error;
pub use http::{Method, StatusCode, Version, header};
pub use tls::Certificate;
pub use url::Url;

use body::{ResponseBody, timeout};
use bytes::Bytes;
use connector::{Connector, NativeHttp};
use futures_core::Stream;
use http::{HeaderMap, HeaderName, HeaderValue, Uri, header::Entry};
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
use hyper_util::{
    client::{legacy::Client as HyperClient, proxy::matcher::Matcher},
    rt::{TokioExecutor, TokioIo, TokioTimer},
};
use std::{future::Future, sync::Arc, task::Poll, time::Duration};
use tokio::time::Instant;

pub fn builder() -> ClientBuilder {
    let mut headers = HeaderMap::with_capacity(2);
    headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));
    ClientBuilder {
        headers: Ok(headers),
        roots: Vec::new(),
        verify_certificates: true,
        proxy: true,
        https_only: false,
        redirect: redirect::Policy::limited(10),
        connect_timeout: None,
        read_timeout: None,
        total_timeout: None,
        keepalive: Duration::from_secs(15),
        max_idle: usize::MAX,
        #[cfg(feature = "test-seams")]
        dns: Default::default(),
    }
}

pub struct ClientBuilder {
    headers: Result<HeaderMap, Error>,
    roots: Vec<Certificate>,
    verify_certificates: bool,
    proxy: bool,
    https_only: bool,
    redirect: redirect::Policy,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    total_timeout: Option<Duration>,
    keepalive: Duration,
    max_idle: usize,
    #[cfg(feature = "test-seams")]
    dns: std::collections::HashMap<String, std::net::SocketAddr>,
}

impl ClientBuilder {
    pub fn no_proxy(mut self) -> Self {
        self.proxy = false;
        self
    }

    pub fn danger_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.verify_certificates = !accept;
        self
    }

    pub fn add_root_certificate(mut self, certificate: Certificate) -> Self {
        self.roots.push(certificate);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.total_timeout = Some(timeout);
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    pub fn tcp_keepalive(mut self, keepalive: Duration) -> Self {
        self.keepalive = keepalive;
        self
    }

    pub fn pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.max_idle = max;
        self
    }

    pub fn https_only(mut self, enabled: bool) -> Self {
        self.https_only = enabled;
        self
    }

    pub fn redirect(mut self, policy: redirect::Policy) -> Self {
        self.redirect = policy;
        self
    }

    pub fn user_agent(mut self, user_agent: &str) -> Self {
        self.headers = self.headers.and_then(|mut headers| {
            headers.insert(
                header::USER_AGENT,
                HeaderValue::from_str(user_agent).map_err(Error::builder)?,
            );
            Ok(headers)
        });
        self
    }

    #[cfg(feature = "test-seams")]
    pub fn resolve(mut self, host: &str, address: std::net::SocketAddr) -> Self {
        self.dns.insert(host.to_ascii_lowercase(), address);
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let matcher = self.proxy.then(Matcher::from_system);
        self.build_with_matcher(matcher)
    }

    fn build_with_matcher(self, matcher: Option<Matcher>) -> Result<Client, Error> {
        let headers = self.headers?;
        let tls = tls::config(self.roots, self.verify_certificates)?;
        #[cfg(feature = "test-seams")]
        let mut http = NativeHttp::new_with_resolver(connector::TestResolver::new(self.dns));
        #[cfg(not(feature = "test-seams"))]
        let mut http = NativeHttp::new();
        http.enforce_http(false);
        http.set_connect_timeout(self.connect_timeout);
        http.set_nodelay(true);
        http.set_keepalive(Some(self.keepalive));
        http.set_keepalive_interval(Some(Duration::from_secs(15)));
        http.set_keepalive_retries(Some(3));
        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        http.set_tcp_user_timeout(Some(Duration::from_secs(30)));

        let connector = Connector::new(
            http,
            tls,
            matcher,
            self.connect_timeout,
            headers.get(header::USER_AGENT).cloned(),
        );
        let hyper = HyperClient::builder(TokioExecutor::new())
            .timer(TokioTimer::new())
            .pool_timer(TokioTimer::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(self.max_idle)
            .build(connector.clone());

        Ok(Client(Arc::new(ClientInner {
            connector,
            hyper,
            redirect: self.redirect,
            headers,
            https_only: self.https_only,
            total_timeout: self.total_timeout,
            read_timeout: self.read_timeout,
        })))
    }
}

struct ClientInner {
    connector: Connector,
    hyper: HyperClient<Connector, Body>,
    redirect: redirect::Policy,
    headers: HeaderMap,
    https_only: bool,
    total_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
}

#[derive(Clone)]
pub struct Client(Arc<ClientInner>);

pub trait IntoUrl {
    fn into_url(self) -> Result<Url, url::ParseError>;
}

impl IntoUrl for Url {
    fn into_url(self) -> Result<Url, url::ParseError> {
        Ok(self)
    }
}

impl IntoUrl for &str {
    fn into_url(self) -> Result<Url, url::ParseError> {
        Url::parse(self)
    }
}

impl IntoUrl for String {
    fn into_url(self) -> Result<Url, url::ParseError> {
        Url::parse(&self)
    }
}

impl IntoUrl for &String {
    fn into_url(self) -> Result<Url, url::ParseError> {
        Url::parse(self)
    }
}

impl Client {
    pub fn get(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    pub fn post(&self, url: impl IntoUrl) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    pub fn request(&self, method: Method, url: impl IntoUrl) -> RequestBuilder {
        let request = url.into_url().map_err(Error::builder).and_then(|mut url| {
            if !url.has_host() {
                return Err(Error::builder("URL scheme is not allowed").with_url(url));
            }
            let mut headers = HeaderMap::new();
            if let Some(auth) = url_auth(&mut url) {
                headers.insert(header::AUTHORIZATION, auth);
            }
            Ok(Request {
                method,
                url,
                headers,
                body: Body::default(),
                timeout: None,
            })
        });
        RequestBuilder {
            client: self.clone(),
            request,
        }
    }
}

fn url_auth(url: &mut Url) -> Option<HeaderValue> {
    let decode = |value: &str| {
        percent_encoding::percent_decode_str(value)
            .decode_utf8()
            .ok()
            .map(|s| s.into_owned())
    };
    let username = decode(url.username())?;
    let password = url.password().and_then(decode);
    if username.is_empty() && password.is_none() {
        return None;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let credentials = format!("{username}:{}", password.unwrap_or_default());
    let encoded = crate::utils::base64::base64_encode(credentials.as_bytes());
    let mut header =
        HeaderValue::from_str(&format!("Basic {encoded}")).expect("base64 is valid ASCII");
    header.set_sensitive(true);
    Some(header)
}

fn allowed_scheme(url: &Url, https_only: bool) -> bool {
    url.scheme() == "https" || (!https_only && url.scheme() == "http")
}

struct Request {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Body,
    timeout: Option<Duration>,
}

pub struct RequestBuilder {
    client: Client,
    request: Result<Request, Error>,
}

impl RequestBuilder {
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<error::BoxError>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<error::BoxError>,
    {
        self.request = self.request.and_then(|mut request| {
            let name = HeaderName::try_from(name).map_err(Error::builder)?;
            let mut value = HeaderValue::try_from(value).map_err(Error::builder)?;
            if request
                .headers
                .get(&name)
                .is_some_and(HeaderValue::is_sensitive)
            {
                value.set_sensitive(true);
            }
            request.headers.append(name, value);
            Ok(request)
        });
        self
    }

    pub fn bearer_auth(mut self, token: impl std::fmt::Display) -> Self {
        self.request = self.request.and_then(|mut request| {
            let mut value =
                HeaderValue::from_str(&format!("Bearer {token}")).map_err(Error::builder)?;
            value.set_sensitive(true);
            request.headers.append(header::AUTHORIZATION, value);
            Ok(request)
        });
        self
    }

    pub fn body(mut self, body: impl Into<Body>) -> Self {
        if let Ok(request) = &mut self.request {
            request.body = body.into();
        }
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        if let Ok(request) = &mut self.request {
            request.timeout = Some(timeout);
        }
        self
    }

    pub fn query<T: serde::Serialize + ?Sized>(mut self, query: &T) -> Self {
        self.request = self.request.and_then(|mut request| {
            let mut pairs = request.url.query_pairs_mut();
            let serializer = serde_urlencoded::Serializer::new(&mut pairs);
            query.serialize(serializer).map_err(Error::builder)?;
            drop(pairs);
            if request.url.query() == Some("") {
                request.url.set_query(None);
            }
            Ok(request)
        });
        self
    }

    pub async fn send(self) -> Result<Response, Error> {
        let Request {
            method,
            mut url,
            mut headers,
            body,
            timeout: request_timeout,
        } = self.request?;
        let client = &self.client.0;
        if !allowed_scheme(&url, client.https_only) {
            return Err(Error::builder("URL scheme is not allowed").with_url(url));
        }
        let uri = url
            .as_str()
            .parse::<Uri>()
            .map_err(|_| Error::builder("URL is not a valid URI").with_url(url.clone()))?;
        for (key, value) in &client.headers {
            if let Entry::Vacant(entry) = headers.entry(key) {
                entry.insert(value.clone());
            }
        }
        let mut request = http::Request::new(body);
        *request.method_mut() = method;
        *request.uri_mut() = uri;
        *request.headers_mut() = headers;

        let total = request_timeout
            .or(client.total_timeout)
            .map(|duration| Instant::now() + duration);
        let headers_deadline = match (total, client.read_timeout) {
            (Some(total), Some(read)) => Some(total.min(Instant::now() + read)),
            (None, Some(read)) => Some(Instant::now() + read),
            (total, None) => total,
        };
        let response = redirect::send(client, request, &url);
        let (response, redirected) = match headers_deadline {
            Some(deadline) => {
                let mut response = std::pin::pin!(response);
                let mut timer = std::pin::pin!(tokio::time::sleep_until(deadline));
                std::future::poll_fn(|cx| {
                    if timer.as_mut().poll(cx).is_ready() {
                        Poll::Ready(Err(Error::request(timeout())))
                    } else {
                        response.as_mut().poll(cx)
                    }
                })
                .await
            }
            None => response.await,
        }
        .map_err(|error| error.with_url(url.clone()))?;

        if let Some(redirected) = redirected {
            url = redirected;
        }
        url.set_fragment(None);
        let (parts, incoming) = response.into_parts();
        Ok(Response {
            inner: http::Response::from_parts(
                parts,
                ResponseBody::new(
                    incoming,
                    total.map(|deadline| Box::pin(tokio::time::sleep_until(deadline))),
                    client.read_timeout,
                ),
            ),
            url: Box::new(url),
        })
    }
}

pub struct Response {
    inner: http::Response<ResponseBody>,
    url: Box<Url>,
}

impl Response {
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    pub fn version(&self) -> Version {
        self.inner.version()
    }

    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn content_length(&self) -> Option<u64> {
        self.inner.body().size_hint().exact()
    }

    pub fn error_for_status(self) -> Result<Self, Error> {
        let status = self.status();
        if status.is_client_error() || status.is_server_error() {
            Err(Error::from_status(status, *self.url))
        } else {
            Ok(self)
        }
    }

    pub async fn chunk(&mut self) -> Result<Option<Bytes>, Error> {
        while let Some(frame) = self.inner.body_mut().frame().await {
            if let Ok(bytes) = frame?.into_data() {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    pub fn bytes_stream(self) -> impl Stream<Item = Result<Bytes, Error>> {
        self.inner.into_body().bytes_stream()
    }

    pub async fn bytes(self) -> Result<Bytes, Error> {
        Ok(self.inner.into_body().collect().await?.to_bytes())
    }

    pub async fn text(self) -> Result<String, Error> {
        Ok(String::from_utf8_lossy(&self.bytes().await?).into_owned())
    }

    pub async fn upgrade(self) -> Result<TokioIo<hyper::upgrade::Upgraded>, Error> {
        hyper::upgrade::on(self.inner)
            .await
            .map(TokioIo::new)
            .map_err(Error::upgrade)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/protocols/http_client.rs"]
mod tests;

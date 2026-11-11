// Redirect handling adapted from tower-http 0.6.11 (https://github.com/tower-rs/tower-http), see LICENSE-tower-http.

use super::{Body, ClientInner, Error, Method, StatusCode, Url, allowed_scheme};
use crate::http::{HeaderMap, HeaderValue, Request, Response, Uri, header};
use crate::http_body::Body as _;
use crate::hyper::body::Incoming;

#[derive(Clone, Copy)]
pub struct Policy(Option<usize>);

impl Policy {
    pub fn none() -> Self {
        Self(None)
    }

    pub fn limited(limit: usize) -> Self {
        Self(Some(limit))
    }
}

pub(super) async fn send(
    client: &ClientInner,
    mut request: Request<Body>,
    url: &Url,
) -> Result<(Response<Incoming>, Option<Url>), Error> {
    let Some(limit) = client.redirect.0 else {
        return Ok((send_once(client, request).await?, None));
    };
    let mut method = request.method().clone();
    let mut headers = request.headers().clone();
    let mut redirected = None;
    let mut count = 0;

    loop {
        let replay = if request.body().size_hint().exact() == Some(0) {
            Some(Body::default())
        } else {
            request.body().try_clone()
        };
        let response = send_once(client, request).await?;
        let body = match response.status() {
            StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND if method == Method::POST => {
                method = Method::GET;
                remove_payload_headers(&mut headers);
                Some(Body::default())
            }
            StatusCode::SEE_OTHER => {
                if method != Method::HEAD {
                    method = Method::GET;
                }
                remove_payload_headers(&mut headers);
                Some(Body::default())
            }
            StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT => replay,
            _ => return Ok((response, redirected)),
        };
        let Some(body) = body else {
            return Ok((response, redirected));
        };
        let previous = redirected.as_ref().unwrap_or(url);
        let location = response.headers().get(header::LOCATION).and_then(|value| {
            let relative = std::str::from_utf8(value.as_bytes()).ok()?;
            let next = previous.join(relative).ok()?;
            let uri = next.as_str().parse::<Uri>().ok()?;
            Some((next, uri))
        });
        let Some((next, uri)) = location else {
            return Ok((response, redirected));
        };
        count += 1;
        if count > limit {
            return Err(Error::redirect("too many redirects").with_url(previous.clone()));
        }
        if !allowed_scheme(&next, client.https_only) {
            return Err(Error::redirect("URL scheme is not allowed").with_url(next));
        }
        if next.scheme() != previous.scheme()
            || next.host_str() != previous.host_str()
            || next.port_or_known_default() != previous.port_or_known_default()
        {
            for name in [
                header::AUTHORIZATION,
                header::COOKIE,
                crate::http::HeaderName::from_static("cookie2"),
                header::PROXY_AUTHORIZATION,
                header::WWW_AUTHENTICATE,
            ] {
                headers.remove(name);
            }
        }

        request = Request::new(body);
        *request.method_mut() = method.clone();
        *request.uri_mut() = uri;
        *request.headers_mut() = headers.clone();
        if !(next.scheme() == "http" && previous.scheme() == "https") {
            let mut referer = previous.clone();
            let _ = referer.set_username("");
            let _ = referer.set_password(None);
            referer.set_fragment(None);
            if let Ok(value) = HeaderValue::from_str(referer.as_str()) {
                request.headers_mut().insert(header::REFERER, value);
            }
        }
        redirected = Some(next);
    }
}

async fn send_once(
    client: &ClientInner,
    request: Request<Body>,
) -> Result<Response<Incoming>, Error> {
    let (mut parts, body) = request.into_parts();
    client
        .connector
        .authorize_proxy(&parts.uri, &mut parts.headers);
    client
        .hyper
        .request(Request::from_parts(parts, body))
        .await
        .map_err(Error::request)
}

fn remove_payload_headers(headers: &mut HeaderMap) {
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_ENCODING,
        header::TRANSFER_ENCODING,
    ] {
        headers.remove(name);
    }
}

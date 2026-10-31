use crate::server::ServerState;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

fn is_potentially_trustworthy(scheme: &str, host: &str) -> bool {
    if scheme == "https" {
        return true;
    }
    let Ok(url) = reqwest::Url::parse(&format!("{scheme}://{host}")) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    host == "localhost"
        || host.ends_with(".localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

pub(crate) async fn apply(
    State(state): State<ServerState>,
    request: Request,
    next: Next,
) -> Response {
    let scheme = request
        .extensions()
        .get::<crate::http::ConnectionScheme>()
        .map_or("http", |scheme| scheme.0);
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let trustworthy = is_potentially_trustworthy(scheme, host);
    let origin = request.headers().get(header::ORIGIN).cloned();
    let mut allow_headers = None;
    if let Some(origin) = &origin {
        let same_origin = origin
            .to_str()
            .ok()
            .and_then(|origin| reqwest::Url::parse(origin).ok())
            .zip(reqwest::Url::parse(&format!("{scheme}://{host}")).ok())
            .is_some_and(|(origin, authority)| {
                origin.scheme() == authority.scheme()
                    && origin.host_str() == authority.host_str()
                    && origin.port_or_known_default() == authority.port_or_known_default()
            });
        if !same_origin
            && !state.prefs.get_or("allow_any_host", false)
            && !state.build_debug.load(std::sync::atomic::Ordering::Relaxed)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        allow_headers = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .and_then(|v| v.to_str().ok())
            .map(|value| {
                value
                    .split(',')
                    .map(|value| value.trim().to_ascii_lowercase())
                    .filter(|value| {
                        matches!(value.as_str(), "content-type" | "authorization" | "accept")
                            || value.starts_with("c-")
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            });
    }
    let options = origin.is_some() && request.method() == Method::OPTIONS;
    let mut response = if options {
        StatusCode::OK.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(
        "x-server-time",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|v| v.as_millis())
            .unwrap_or_default()
            .to_string()
            .parse()
            .unwrap(),
    );
    if trustworthy {
        headers.insert("cross-origin-opener-policy", "same-origin".parse().unwrap());
        headers.insert(
            "cross-origin-embedder-policy",
            "credentialless".parse().unwrap(),
        );
    }
    if let Some(origin) = origin {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.append(header::VARY, "Origin".parse().unwrap());
        if options {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET, POST, PUT, DELETE, OPTIONS, HEAD".parse().unwrap(),
            );
            headers.insert(header::ACCESS_CONTROL_MAX_AGE, "86400".parse().unwrap());
            if let Some(value) = allow_headers.filter(|v| !v.is_empty()) {
                if let Ok(value) = value.parse() {
                    headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, value);
                }
            }
        }
    }
    response
}

#[cfg(test)]
#[path = "../../../tests/unit/http/auth/cors.rs"]
mod tests;

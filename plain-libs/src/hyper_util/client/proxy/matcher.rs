use self::builder::IntoValue;
use crate::http::header::HeaderValue;
use crate::ipnet::IpNet;
use percent_encoding::percent_decode_str;
use std::fmt;
use std::net::IpAddr;
pub struct Matcher {
    http: Option<Intercept>,
    https: Option<Intercept>,
    no: NoProxy,
}
#[derive(Clone)]
pub struct Intercept {
    uri: crate::http::Uri,
    auth: Auth,
}
#[derive(Default)]
pub struct Builder {
    is_cgi: bool,
    all: String,
    http: String,
    https: String,
    no: String,
}
#[derive(Clone)]
enum Auth {
    Empty,
    Basic(crate::http::header::HeaderValue),
    Raw(String, String),
}
#[derive(Clone, Debug, Default)]
struct NoProxy {
    ips: IpMatcher,
    domains: DomainMatcher,
}
#[derive(Clone, Debug, Default)]
struct DomainMatcher(Vec<String>);
#[derive(Clone, Debug, Default)]
struct IpMatcher(Vec<Ip>);
#[derive(Clone, Debug)]
enum Ip {
    Address(IpAddr),
    Network(IpNet),
}
impl Matcher {
    pub fn from_env() -> Self {
        Builder::from_env().build()
    }
    pub fn from_system() -> Self {
        Builder::from_system().build()
    }
    pub fn builder() -> Builder {
        Builder::default()
    }
    pub fn intercept(&self, dst: &crate::http::Uri) -> Option<Intercept> {
        if self.no.contains(dst.host()?) {
            return None;
        }
        match dst.scheme_str() {
            Some("http") => self.http.clone(),
            Some("https") => self.https.clone(),
            _ => None,
        }
    }
}
impl fmt::Debug for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = f.debug_struct("Matcher");
        if let Some(ref http) = self.http {
            b.field("http", http);
        }
        if let Some(ref https) = self.https {
            b.field("https", https);
        }
        if !self.no.is_empty() {
            b.field("no", &self.no);
        }
        b.finish()
    }
}
impl Intercept {
    pub fn uri(&self) -> &crate::http::Uri {
        &self.uri
    }
    pub fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Auth::Basic(ref val) = self.auth {
            Some(val)
        } else {
            None
        }
    }
}
impl fmt::Debug for Intercept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Intercept").field("uri", &self.uri).finish()
    }
}
impl Builder {
    fn from_env() -> Self {
        Builder {
            is_cgi: std::env::var_os("REQUEST_METHOD").is_some(),
            all: get_first_env(&["ALL_PROXY", "all_proxy"]),
            http: get_first_env(&["HTTP_PROXY", "http_proxy"]),
            https: get_first_env(&["HTTPS_PROXY", "https_proxy"]),
            no: get_first_env(&["NO_PROXY", "no_proxy"]),
        }
    }
    fn from_system() -> Self {
        let mut builder = Self::from_env();
        builder
    }
    pub fn all<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.all = val.into_value();
        self
    }
    pub fn http<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.http = val.into_value();
        self
    }
    pub fn https<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.https = val.into_value();
        self
    }
    pub fn no<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.no = val.into_value();
        self
    }
    pub fn build(self) -> Matcher {
        if self.is_cgi {
            return Matcher {
                http: None,
                https: None,
                no: NoProxy::empty(),
            };
        }
        let all = parse_env_uri(&self.all);
        Matcher {
            http: parse_env_uri(&self.http).or_else(|| all.clone()),
            https: parse_env_uri(&self.https).or(all),
            no: NoProxy::from_string(&self.no),
        }
    }
}
fn get_first_env(names: &[&str]) -> String {
    for name in names {
        if let Ok(val) = std::env::var(name) {
            return val;
        }
    }
    String::new()
}
fn parse_env_uri(val: &str) -> Option<Intercept> {
    use std::borrow::Cow;
    let uri = val.parse::<crate::http::Uri>().ok()?;
    let mut builder = crate::http::Uri::builder();
    let mut is_httpish = false;
    let mut auth = Auth::Empty;
    builder = builder.scheme(match uri.scheme() {
        Some(s) => {
            if s == &crate::http::uri::Scheme::HTTP || s == &crate::http::uri::Scheme::HTTPS {
                is_httpish = true;
                s.clone()
            } else if matches!(s.as_str(), "socks4" | "socks4a" | "socks5" | "socks5h") {
                s.clone()
            } else {
                return None;
            }
        }
        None => {
            is_httpish = true;
            crate::http::uri::Scheme::HTTP
        }
    });
    let authority = uri.authority()?;
    if let Some((userinfo, host_port)) = authority.as_str().split_once('@') {
        let (user, pass) = match userinfo.split_once(':') {
            Some((user, pass)) => (user, Some(pass)),
            None => (userinfo, None),
        };
        let user = percent_decode_str(user).decode_utf8_lossy();
        let pass = pass.map(|pass| percent_decode_str(pass).decode_utf8_lossy());
        if is_httpish {
            auth = Auth::Basic(encode_basic_auth(&user, pass.as_deref()));
        } else {
            auth = Auth::Raw(
                user.into_owned(),
                pass.map_or_else(String::new, Cow::into_owned),
            );
        }
        builder = builder.authority(host_port);
    } else {
        builder = builder.authority(authority.clone());
    }
    builder = builder.path_and_query("/");
    let dst = builder.build().ok()?;
    Some(Intercept { uri: dst, auth })
}

impl NoProxy {
    fn empty() -> NoProxy {
        NoProxy {
            ips: IpMatcher(Vec::new()),
            domains: DomainMatcher(Vec::new()),
        }
    }
    pub fn from_string(no_proxy_list: &str) -> Self {
        let mut ips = Vec::new();
        let mut domains = Vec::new();
        let parts = no_proxy_list.split(',').map(str::trim);
        for part in parts {
            match part.parse::<IpNet>() {
                Ok(ip) => ips.push(Ip::Network(ip)),
                Err(_) => match part.parse::<IpAddr>() {
                    Ok(addr) => ips.push(Ip::Address(addr)),
                    Err(_) => {
                        if !part.trim().is_empty() {
                            domains.push(part.to_owned())
                        }
                    }
                },
            }
        }
        NoProxy {
            ips: IpMatcher(ips),
            domains: DomainMatcher(domains),
        }
    }
    pub fn contains(&self, host: &str) -> bool {
        let host = crate::hyper_util::client::strip_ipv6_brackets(host);
        match host.parse::<IpAddr>() {
            Ok(ip) => self.ips.contains(ip),
            Err(_) => self.domains.contains(host),
        }
    }
    fn is_empty(&self) -> bool {
        self.ips.0.is_empty() && self.domains.0.is_empty()
    }
}
impl IpMatcher {
    fn contains(&self, addr: IpAddr) -> bool {
        for ip in &self.0 {
            match ip {
                Ip::Address(address) => {
                    if &addr == address {
                        return true;
                    }
                }
                Ip::Network(net) => {
                    if net.contains(&addr) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
impl DomainMatcher {
    fn contains(&self, domain: &str) -> bool {
        let domain_len = domain.len();
        for d in &self.0 {
            if d.eq_ignore_ascii_case(domain)
                || d.strip_prefix('.')
                    .map_or(false, |s| s.eq_ignore_ascii_case(domain))
            {
                return true;
            } else if domain
                .get(domain_len.saturating_sub(d.len())..)
                .map_or(false, |s| s.eq_ignore_ascii_case(d))
            {
                if d.starts_with('.') {
                    return true;
                } else if domain.as_bytes().get(domain_len - d.len() - 1) == Some(&b'.') {
                    return true;
                }
            } else if d == "*" {
                return true;
            }
        }
        false
    }
}
mod builder {
    pub trait IntoValue {
        fn into_value(self) -> String;
    }
    impl IntoValue for String {
        fn into_value(self) -> String {
            self
        }
    }
    impl IntoValue for &String {
        fn into_value(self) -> String {
            self.into()
        }
    }
    impl IntoValue for &str {
        fn into_value(self) -> String {
            self.into()
        }
    }
}

fn encode_basic_auth(user: &str, pass: Option<&str>) -> HeaderValue {
    let password = pass.unwrap_or("");
    let mut buf = Vec::with_capacity(6 + (user.len() + 1 + password.len()).div_ceil(3) * 4);
    buf.extend_from_slice(b"Basic ");
    crate::utils::base64::base64_encode_append_segments(
        &mut buf,
        &[user.as_bytes(), b":", password.as_bytes()],
    );
    #[cfg(test)]
    tests::record_auth_buffer(buf.as_ptr() as usize);
    let mut header = HeaderValue::from_maybe_shared(bytes::Bytes::from(buf))
        .expect("base64 is always valid HeaderValue");
    header.set_sensitive(true);
    header
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/hyper_util/client/proxy/matcher.rs"]
mod tests;

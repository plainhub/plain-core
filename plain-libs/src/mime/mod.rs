use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::slice;
use std::str::FromStr;
mod parse;
#[derive(Clone)]
pub struct Mime {
    source: Source,
    slash: usize,
    plus: Option<usize>,
    params: ParamSource,
}
#[derive(Clone, Debug)]
pub struct MimeIter<'a> {
    pos: usize,
    source: &'a str,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name<'a> {
    source: &'a str,
    insensitive: bool,
}
#[derive(Debug)]
pub struct FromStrError {
    inner: parse::ParseError,
}
impl FromStrError {
    fn s(&self) -> &str {
        "mime parse error"
    }
}
impl fmt::Display for FromStrError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.s(), self.inner)
    }
}
impl Error for FromStrError {
    fn description(&self) -> &str {
        self.s()
    }
}
#[derive(Clone)]
enum Source {
    Atom(u8, &'static str),
    Dynamic(String),
}
impl Source {
    fn as_ref(&self) -> &str {
        match *self {
            Source::Atom(_, s) => s,
            Source::Dynamic(ref s) => s,
        }
    }
}
#[derive(Clone)]
enum ParamSource {
    Utf8(usize),
    Custom(usize, Vec<(Indexed, Indexed)>),
    None,
}
#[derive(Clone, Copy)]
struct Indexed(usize, usize);
impl Mime {
    #[inline]
    pub fn type_(&self) -> Name {
        Name {
            source: &self.source.as_ref()[..self.slash],
            insensitive: true,
        }
    }
    #[inline]
    pub fn subtype(&self) -> Name {
        let end = self
            .plus
            .unwrap_or_else(|| return self.semicolon().unwrap_or(self.source.as_ref().len()));
        Name {
            source: &self.source.as_ref()[self.slash + 1..end],
            insensitive: true,
        }
    }
    #[inline]
    pub fn suffix(&self) -> Option<Name> {
        let end = self.semicolon().unwrap_or(self.source.as_ref().len());
        self.plus.map(|idx| Name {
            source: &self.source.as_ref()[idx + 1..end],
            insensitive: true,
        })
    }
    pub fn get_param<'a, N>(&'a self, attr: N) -> Option<Name<'a>>
    where
        N: PartialEq<Name<'a>>,
    {
        self.params().find(|e| attr == e.0).map(|e| e.1)
    }
    #[inline]
    pub fn params<'a>(&'a self) -> Params<'a> {
        let inner = match self.params {
            ParamSource::Utf8(_) => ParamsInner::Utf8,
            ParamSource::Custom(_, ref params) => ParamsInner::Custom {
                source: &self.source,
                params: params.iter(),
            },
            ParamSource::None => ParamsInner::None,
        };
        Params(inner)
    }

    #[inline]
    fn semicolon(&self) -> Option<usize> {
        match self.params {
            ParamSource::Utf8(i) | ParamSource::Custom(i, _) => Some(i),
            ParamSource::None => None,
        }
    }
    fn atom(&self) -> u8 {
        match self.source {
            Source::Atom(a, _) => a,
            _ => 0,
        }
    }
}
fn eq_ascii(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}
fn mime_eq_str(mime: &Mime, s: &str) -> bool {
    if let ParamSource::Utf8(semicolon) = mime.params {
        if mime.source.as_ref().len() == s.len() {
            eq_ascii(mime.source.as_ref(), s)
        } else {
            params_eq(semicolon, mime.source.as_ref(), s)
        }
    } else if let Some(semicolon) = mime.semicolon() {
        params_eq(semicolon, mime.source.as_ref(), s)
    } else {
        eq_ascii(mime.source.as_ref(), s)
    }
}
fn params_eq(semicolon: usize, a: &str, b: &str) -> bool {
    if b.len() < semicolon + 1 {
        false
    } else if !eq_ascii(&a[..semicolon], &b[..semicolon]) {
        false
    } else {
        let mut a = &a[semicolon + 1..];
        let mut b = &b[semicolon + 1..];
        let mut sensitive;
        loop {
            a = a.trim();
            b = b.trim();
            match (a.is_empty(), b.is_empty()) {
                (true, true) => return true,
                (true, false) | (false, true) => return false,
                (false, false) => (),
            }
            if let Some(a_idx) = a.find('=') {
                let a_name = { { a[..a_idx].trim_start() } };
                if let Some(b_idx) = b.find('=') {
                    let b_name = { { b[..b_idx].trim_start() } };
                    if !eq_ascii(a_name, b_name) {
                        return false;
                    }
                    sensitive = a_name != CHARSET;
                    a = &a[..a_idx];
                    b = &b[..b_idx];
                } else {
                    return false;
                }
            } else {
                return false;
            }
            let a_quoted = if a.as_bytes()[0] == b'"' {
                a = &a[1..];
                true
            } else {
                false
            };
            let b_quoted = if b.as_bytes()[0] == b'"' {
                b = &b[1..];
                true
            } else {
                false
            };
            let a_end = if a_quoted {
                if let Some(quote) = a.find('"') {
                    quote
                } else {
                    return false;
                }
            } else {
                a.find(';').unwrap_or(a.len())
            };
            let b_end = if b_quoted {
                if let Some(quote) = b.find('"') {
                    quote
                } else {
                    return false;
                }
            } else {
                b.find(';').unwrap_or(b.len())
            };
            if sensitive {
                if !eq_ascii(&a[..a_end], &b[..b_end]) {
                    return false;
                }
            } else {
                if &a[..a_end] != &b[..b_end] {
                    return false;
                }
            }
            a = &a[a_end..];
            b = &b[b_end..];
        }
    }
}
impl PartialEq for Mime {
    #[inline]
    fn eq(&self, other: &Mime) -> bool {
        match (self.atom(), other.atom()) {
            (0, _) | (_, 0) => mime_eq_str(self, other.source.as_ref()),
            (a, b) => a == b,
        }
    }
}
impl Eq for Mime {}
impl PartialOrd for Mime {
    fn partial_cmp(&self, other: &Mime) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Mime {
    fn cmp(&self, other: &Mime) -> Ordering {
        self.source.as_ref().cmp(other.source.as_ref())
    }
}
impl Hash for Mime {
    fn hash<T: Hasher>(&self, hasher: &mut T) {
        hasher.write(self.source.as_ref().as_bytes());
    }
}
impl<'a> PartialEq<&'a str> for Mime {
    #[inline]
    fn eq(&self, s: &&'a str) -> bool {
        mime_eq_str(self, *s)
    }
}
impl<'a> PartialEq<Mime> for &'a str {
    #[inline]
    fn eq(&self, mime: &Mime) -> bool {
        mime_eq_str(mime, *self)
    }
}
impl FromStr for Mime {
    type Err = FromStrError;
    fn from_str(s: &str) -> Result<Mime, Self::Err> {
        parse::parse(s).map_err(|e| FromStrError { inner: e })
    }
}
impl AsRef<str> for Mime {
    #[inline]
    fn as_ref(&self) -> &str {
        self.source.as_ref()
    }
}
impl fmt::Debug for Mime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.source.as_ref(), f)
    }
}
impl fmt::Display for Mime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.source.as_ref(), f)
    }
}
fn name_eq_str(name: &Name, s: &str) -> bool {
    if name.insensitive {
        eq_ascii(name.source, s)
    } else {
        name.source == s
    }
}
impl<'a> Name<'a> {
    pub fn as_str(&self) -> &'a str {
        self.source
    }
}
impl<'a, 'b> PartialEq<&'b str> for Name<'a> {
    #[inline]
    fn eq(&self, other: &&'b str) -> bool {
        name_eq_str(self, *other)
    }
}
impl<'a, 'b> PartialEq<Name<'a>> for &'b str {
    #[inline]
    fn eq(&self, other: &Name<'a>) -> bool {
        name_eq_str(other, *self)
    }
}
impl<'a> AsRef<str> for Name<'a> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.source
    }
}
impl<'a> From<Name<'a>> for &'a str {
    #[inline]
    fn from(name: Name<'a>) -> &'a str {
        name.source
    }
}
impl<'a> fmt::Debug for Name<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.source, f)
    }
}
impl<'a> fmt::Display for Name<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.source, f)
    }
}
enum ParamsInner<'a> {
    Utf8,
    Custom {
        source: &'a Source,
        params: slice::Iter<'a, (Indexed, Indexed)>,
    },
    None,
}
pub struct Params<'a>(ParamsInner<'a>);
impl<'a> fmt::Debug for Params<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Params").finish()
    }
}
impl<'a> Iterator for Params<'a> {
    type Item = (Name<'a>, Name<'a>);
    #[inline]
    fn next(&mut self) -> Option<(Name<'a>, Name<'a>)> {
        match self.0 {
            ParamsInner::Utf8 => {
                let value = (CHARSET, UTF_8);
                self.0 = ParamsInner::None;
                Some(value)
            }
            ParamsInner::Custom {
                source,
                ref mut params,
            } => params.next().map(|&(name, value)| {
                let name = Name {
                    source: &source.as_ref()[name.0..name.1],
                    insensitive: true,
                };
                let value = Name {
                    source: &source.as_ref()[value.0..value.1],
                    insensitive: name == CHARSET,
                };
                (name, value)
            }),
            ParamsInner::None => None,
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.0 {
            ParamsInner::Utf8 => (1, Some(1)),
            ParamsInner::Custom { ref params, .. } => params.size_hint(),
            ParamsInner::None => (0, Some(0)),
        }
    }
}
macro_rules! names { ($($id : ident , $e : expr ;) *) => ($(#[doc = $e] pub const $id : Name <'static > = Name { source : $e , insensitive : true , } ;) * ) }
names! { STAR , "*" ; TEXT , "text" ; IMAGE , "image" ; AUDIO , "audio" ; VIDEO , "video" ; APPLICATION , "application" ; MULTIPART , "multipart" ; MESSAGE , "message" ; MODEL , "model" ; FONT , "font" ; PLAIN , "plain" ; HTML , "html" ; XML , "xml" ; JAVASCRIPT , "javascript" ; CSS , "css" ; CSV , "csv" ; EVENT_STREAM , "event-stream" ; VCARD , "vcard" ; JSON , "json" ; WWW_FORM_URLENCODED , "x-www-form-urlencoded" ; MSGPACK , "msgpack" ; OCTET_STREAM , "octet-stream" ; PDF , "pdf" ; WOFF , "woff" ; WOFF2 , "woff2" ; FORM_DATA , "form-data" ; BMP , "bmp" ; GIF , "gif" ; JPEG , "jpeg" ; PNG , "png" ; SVG , "svg" ; BASIC , "basic" ; MPEG , "mpeg" ; MP4 , "mp4" ; OGG , "ogg" ; CHARSET , "charset" ; BOUNDARY , "boundary" ; UTF_8 , "utf-8" ; }
macro_rules! mimes { ($($id : ident , $variant : ident , $($piece : expr) ,*;) *) => (enum Atoms { Dynamic , $($variant ,) * } $(mime_constant ! { $id , $variant , $($piece) ,* }) * ) }
macro_rules! mime_constant { ($id : ident , $variant : ident , $src : expr , $slash : expr) => (mime_constant ! ($id , $variant , $src , $slash , None) ;) ; ($id : ident , $variant : ident , $src : expr , $slash : expr , $plus : expr) => (mime_constant ! (FULL $id , $variant , $src , $slash , $plus , ParamSource::None) ;) ; ($id : ident , $variant : ident , $src : expr , $slash : expr , $plus : expr , $params : expr) => (mime_constant ! (FULL $id , $variant , $src , $slash , $plus , ParamSource::Utf8 ($params)) ;) ; (FULL $id : ident , $variant : ident , $src : expr , $slash : expr , $plus : expr , $params : expr) => (#[doc = $src] pub const $id : Mime = Mime { source : Source::Atom (Atoms::$variant as u8 , $src) , slash : $slash , plus : $plus , params : $params , } ;) }
mimes! { STAR_STAR, StarStar, "*/*" , 1 ; TEXT_STAR, TextStar, "text/*" , 4 ; TEXT_PLAIN, TextPlain, "text/plain" , 4 ; TEXT_PLAIN_UTF_8, TextPlainUtf8, "text/plain; charset=utf-8" , 4 , None , 10 ; TEXT_HTML, TextHtml, "text/html" , 4 ; TEXT_HTML_UTF_8, TextHtmlUtf8, "text/html; charset=utf-8" , 4 , None , 9 ; TEXT_CSS, TextCss, "text/css" , 4 ; TEXT_CSS_UTF_8, TextCssUtf8, "text/css; charset=utf-8" , 4 , None , 8 ; TEXT_JAVASCRIPT, TextJavascript, "text/javascript" , 4 ; TEXT_XML, TextXml, "text/xml" , 4 ; TEXT_EVENT_STREAM, TextEventStream, "text/event-stream" , 4 ; TEXT_CSV, TextCsv, "text/csv" , 4 ; TEXT_CSV_UTF_8, TextCsvUtf8, "text/csv; charset=utf-8" , 4 , None , 8 ; TEXT_TAB_SEPARATED_VALUES, TextTabSeparatedValues, "text/tab-separated-values" , 4 ; TEXT_TAB_SEPARATED_VALUES_UTF_8, TextTabSeparatedValuesUtf8, "text/tab-separated-values; charset=utf-8" , 4 , None , 25 ; TEXT_VCARD, TextVcard, "text/vcard" , 4 ; IMAGE_STAR, ImageStar, "image/*" , 5 ; IMAGE_JPEG, ImageJpeg, "image/jpeg" , 5 ; IMAGE_GIF, ImageGif, "image/gif" , 5 ; IMAGE_PNG, ImagePng, "image/png" , 5 ; IMAGE_BMP, ImageBmp, "image/bmp" , 5 ; IMAGE_SVG, ImageSvg, "image/svg+xml" , 5 , Some (9) ; FONT_WOFF, FontWoff, "font/woff" , 4 ; FONT_WOFF2, FontWoff2, "font/woff2" , 4 ; APPLICATION_JSON, ApplicationJson, "application/json" , 11 ; APPLICATION_JAVASCRIPT, ApplicationJavascript, "application/javascript" , 11 ; APPLICATION_JAVASCRIPT_UTF_8, ApplicationJavascriptUtf8, "application/javascript; charset=utf-8" , 11 , None , 22 ; APPLICATION_WWW_FORM_URLENCODED, ApplicationWwwFormUrlencoded, "application/x-www-form-urlencoded" , 11 ; APPLICATION_OCTET_STREAM, ApplicationOctetStream, "application/octet-stream" , 11 ; APPLICATION_MSGPACK, ApplicationMsgpack, "application/msgpack" , 11 ; APPLICATION_PDF, ApplicationPdf, "application/pdf" , 11 ; MULTIPART_FORM_DATA, MultipartFormData, "multipart/form-data" , 9 ; }

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/mime.rs"]
mod tests;

use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;
use std::str::FromStr;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatusCode(NonZeroU16);
pub struct InvalidStatusCode {
    _priv: (),
}
impl StatusCode {
    #[inline]
    pub const fn from_u16(src: u16) -> Result<StatusCode, InvalidStatusCode> {
        if let 100..=999 = src {
            if let Some(code) = NonZeroU16::new(src) {
                return Ok(StatusCode(code));
            }
        }
        Err(InvalidStatusCode::new())
    }
    pub fn from_bytes(src: &[u8]) -> Result<StatusCode, InvalidStatusCode> {
        if src.len() != 3 {
            return Err(InvalidStatusCode::new());
        }
        let a = src[0].wrapping_sub(b'0') as u16;
        let b = src[1].wrapping_sub(b'0') as u16;
        let c = src[2].wrapping_sub(b'0') as u16;
        if a == 0 || a > 9 || b > 9 || c > 9 {
            return Err(InvalidStatusCode::new());
        }
        let status = (a * 100) + (b * 10) + c;
        NonZeroU16::new(status)
            .map(StatusCode)
            .ok_or_else(InvalidStatusCode::new)
    }
    #[inline]
    pub const fn as_u16(&self) -> u16 {
        self.0.get()
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        let offset = (self.0.get() - 100) as usize;
        let offset = offset * 3;
        #[cfg(debug_assertions)]
        {
            &CODE_DIGITS[offset..offset + 3]
        }
        #[cfg(not(debug_assertions))]
        unsafe {
            CODE_DIGITS.get_unchecked(offset..offset + 3)
        }
    }
    pub fn canonical_reason(&self) -> Option<&'static str> {
        canonical_reason(self.0.get())
    }
    #[inline]
    pub fn is_informational(&self) -> bool {
        (100..200).contains(&self.0.get())
    }
    #[inline]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.0.get())
    }
    #[inline]
    pub fn is_redirection(&self) -> bool {
        (300..400).contains(&self.0.get())
    }
    #[inline]
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.0.get())
    }
    #[inline]
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.0.get())
    }
}
impl fmt::Debug for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            u16::from(*self),
            self.canonical_reason().unwrap_or("<unknown status code>")
        )
    }
}
impl Default for StatusCode {
    #[inline]
    fn default() -> StatusCode {
        StatusCode::OK
    }
}
impl PartialEq<u16> for StatusCode {
    #[inline]
    fn eq(&self, other: &u16) -> bool {
        self.as_u16() == *other
    }
}
impl PartialEq<StatusCode> for u16 {
    #[inline]
    fn eq(&self, other: &StatusCode) -> bool {
        *self == other.as_u16()
    }
}
impl From<StatusCode> for u16 {
    #[inline]
    fn from(status: StatusCode) -> u16 {
        status.0.get()
    }
}
impl FromStr for StatusCode {
    type Err = InvalidStatusCode;
    fn from_str(s: &str) -> Result<StatusCode, InvalidStatusCode> {
        StatusCode::from_bytes(s.as_ref())
    }
}
impl From<&StatusCode> for StatusCode {
    #[inline]
    fn from(t: &StatusCode) -> Self {
        t.to_owned()
    }
}
impl TryFrom<&[u8]> for StatusCode {
    type Error = InvalidStatusCode;
    #[inline]
    fn try_from(t: &[u8]) -> Result<Self, Self::Error> {
        StatusCode::from_bytes(t)
    }
}
impl TryFrom<&str> for StatusCode {
    type Error = InvalidStatusCode;
    #[inline]
    fn try_from(t: &str) -> Result<Self, Self::Error> {
        t.parse()
    }
}
impl TryFrom<u16> for StatusCode {
    type Error = InvalidStatusCode;
    #[inline]
    fn try_from(t: u16) -> Result<Self, Self::Error> {
        StatusCode::from_u16(t)
    }
}
macro_rules! status_codes { ($($(#[$docs : meta]) * ($num : expr , $konst : ident , $phrase : expr) ;) +) => { impl StatusCode { $($(#[$docs]) * pub const $konst : StatusCode = StatusCode (unsafe { NonZeroU16::new_unchecked ($num) }) ;) + } fn canonical_reason (num : u16) -> Option <&'static str > { match num { $($num => Some ($phrase) ,) + _ => None } } } }
status_codes! { (100 , CONTINUE , "Continue") ; (101 , SWITCHING_PROTOCOLS , "Switching Protocols") ; (102 , PROCESSING , "Processing") ; (103 , EARLY_HINTS , "Early Hints") ; (200 , OK , "OK") ; (201 , CREATED , "Created") ; (202 , ACCEPTED , "Accepted") ; (203 , NON_AUTHORITATIVE_INFORMATION , "Non Authoritative Information") ; (204 , NO_CONTENT , "No Content") ; (205 , RESET_CONTENT , "Reset Content") ; (206 , PARTIAL_CONTENT , "Partial Content") ; (207 , MULTI_STATUS , "Multi-Status") ; (208 , ALREADY_REPORTED , "Already Reported") ; (226 , IM_USED , "IM Used") ; (300 , MULTIPLE_CHOICES , "Multiple Choices") ; (301 , MOVED_PERMANENTLY , "Moved Permanently") ; (302 , FOUND , "Found") ; (303 , SEE_OTHER , "See Other") ; (304 , NOT_MODIFIED , "Not Modified") ; (305 , USE_PROXY , "Use Proxy") ; (307 , TEMPORARY_REDIRECT , "Temporary Redirect") ; (308 , PERMANENT_REDIRECT , "Permanent Redirect") ; (400 , BAD_REQUEST , "Bad Request") ; (401 , UNAUTHORIZED , "Unauthorized") ; (402 , PAYMENT_REQUIRED , "Payment Required") ; (403 , FORBIDDEN , "Forbidden") ; (404 , NOT_FOUND , "Not Found") ; (405 , METHOD_NOT_ALLOWED , "Method Not Allowed") ; (406 , NOT_ACCEPTABLE , "Not Acceptable") ; (407 , PROXY_AUTHENTICATION_REQUIRED , "Proxy Authentication Required") ; (408 , REQUEST_TIMEOUT , "Request Timeout") ; (409 , CONFLICT , "Conflict") ; (410 , GONE , "Gone") ; (411 , LENGTH_REQUIRED , "Length Required") ; (412 , PRECONDITION_FAILED , "Precondition Failed") ; (413 , PAYLOAD_TOO_LARGE , "Payload Too Large") ; (414 , URI_TOO_LONG , "URI Too Long") ; (415 , UNSUPPORTED_MEDIA_TYPE , "Unsupported Media Type") ; (416 , RANGE_NOT_SATISFIABLE , "Range Not Satisfiable") ; (417 , EXPECTATION_FAILED , "Expectation Failed") ; (418 , IM_A_TEAPOT , "I'm a teapot") ; (421 , MISDIRECTED_REQUEST , "Misdirected Request") ; (422 , UNPROCESSABLE_ENTITY , "Unprocessable Entity") ; (423 , LOCKED , "Locked") ; (424 , FAILED_DEPENDENCY , "Failed Dependency") ; (425 , TOO_EARLY , "Too Early") ; (426 , UPGRADE_REQUIRED , "Upgrade Required") ; (428 , PRECONDITION_REQUIRED , "Precondition Required") ; (429 , TOO_MANY_REQUESTS , "Too Many Requests") ; (431 , REQUEST_HEADER_FIELDS_TOO_LARGE , "Request Header Fields Too Large") ; (451 , UNAVAILABLE_FOR_LEGAL_REASONS , "Unavailable For Legal Reasons") ; (500 , INTERNAL_SERVER_ERROR , "Internal Server Error") ; (501 , NOT_IMPLEMENTED , "Not Implemented") ; (502 , BAD_GATEWAY , "Bad Gateway") ; (503 , SERVICE_UNAVAILABLE , "Service Unavailable") ; (504 , GATEWAY_TIMEOUT , "Gateway Timeout") ; (505 , HTTP_VERSION_NOT_SUPPORTED , "HTTP Version Not Supported") ; (506 , VARIANT_ALSO_NEGOTIATES , "Variant Also Negotiates") ; (507 , INSUFFICIENT_STORAGE , "Insufficient Storage") ; (508 , LOOP_DETECTED , "Loop Detected") ; (510 , NOT_EXTENDED , "Not Extended") ; (511 , NETWORK_AUTHENTICATION_REQUIRED , "Network Authentication Required") ; }
impl InvalidStatusCode {
    const fn new() -> InvalidStatusCode {
        InvalidStatusCode { _priv: () }
    }
}
impl fmt::Debug for InvalidStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InvalidStatusCode").finish()
    }
}
impl fmt::Display for InvalidStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid status code")
    }
}
impl Error for InvalidStatusCode {}
const CODE_DIGITS: &str = "\
100101102103104105106107108109110111112113114115116117118119\
120121122123124125126127128129130131132133134135136137138139\
140141142143144145146147148149150151152153154155156157158159\
160161162163164165166167168169170171172173174175176177178179\
180181182183184185186187188189190191192193194195196197198199\
200201202203204205206207208209210211212213214215216217218219\
220221222223224225226227228229230231232233234235236237238239\
240241242243244245246247248249250251252253254255256257258259\
260261262263264265266267268269270271272273274275276277278279\
280281282283284285286287288289290291292293294295296297298299\
300301302303304305306307308309310311312313314315316317318319\
320321322323324325326327328329330331332333334335336337338339\
340341342343344345346347348349350351352353354355356357358359\
360361362363364365366367368369370371372373374375376377378379\
380381382383384385386387388389390391392393394395396397398399\
400401402403404405406407408409410411412413414415416417418419\
420421422423424425426427428429430431432433434435436437438439\
440441442443444445446447448449450451452453454455456457458459\
460461462463464465466467468469470471472473474475476477478479\
480481482483484485486487488489490491492493494495496497498499\
500501502503504505506507508509510511512513514515516517518519\
520521522523524525526527528529530531532533534535536537538539\
540541542543544545546547548549550551552553554555556557558559\
560561562563564565566567568569570571572573574575576577578579\
580581582583584585586587588589590591592593594595596597598599\
600601602603604605606607608609610611612613614615616617618619\
620621622623624625626627628629630631632633634635636637638639\
640641642643644645646647648649650651652653654655656657658659\
660661662663664665666667668669670671672673674675676677678679\
680681682683684685686687688689690691692693694695696697698699\
700701702703704705706707708709710711712713714715716717718719\
720721722723724725726727728729730731732733734735736737738739\
740741742743744745746747748749750751752753754755756757758759\
760761762763764765766767768769770771772773774775776777778779\
780781782783784785786787788789790791792793794795796797798799\
800801802803804805806807808809810811812813814815816817818819\
820821822823824825826827828829830831832833834835836837838839\
840841842843844845846847848849850851852853854855856857858859\
860861862863864865866867868869870871872873874875876877878879\
880881882883884885886887888889890891892893894895896897898899\
900901902903904905906907908909910911912913914915916917918919\
920921922923924925926927928929930931932933934935936937938939\
940941942943944945946947948949950951952953954955956957958959\
960961962963964965966967968969970971972973974975976977978979\
980981982983984985986987988989990991992993994995996997998999";

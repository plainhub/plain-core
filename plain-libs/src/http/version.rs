use std::fmt;
#[derive(PartialEq, PartialOrd, Copy, Clone, Eq, Ord, Hash)]
pub struct Version(Http);
impl Version {
    pub const HTTP_09: Version = Version(Http::Http09);
    pub const HTTP_10: Version = Version(Http::Http10);
    pub const HTTP_11: Version = Version(Http::Http11);
    pub const HTTP_2: Version = Version(Http::H2);
    pub const HTTP_3: Version = Version(Http::H3);
}
#[derive(PartialEq, PartialOrd, Copy, Clone, Eq, Ord, Hash)]
enum Http {
    Http09,
    Http10,
    Http11,
    H2,
    H3,
    __NonExhaustive,
}
impl Default for Version {
    #[inline]
    fn default() -> Version {
        Version::HTTP_11
    }
}
impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Http::*;
        f.write_str(match self.0 {
            Http09 => "HTTP/0.9",
            Http10 => "HTTP/1.0",
            Http11 => "HTTP/1.1",
            H2 => "HTTP/2.0",
            H3 => "HTTP/3.0",
            __NonExhaustive => unreachable!(),
        })
    }
}

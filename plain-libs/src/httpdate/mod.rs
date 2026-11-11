#![forbid(unsafe_code)]
pub use date::HttpDate;
use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::time::SystemTime;
mod date;
#[derive(Debug)]
pub struct Error(());
impl error::Error for Error {}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_str("string contains no or an invalid date")
    }
}
impl From<Error> for io::Error {
    fn from(e: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }
}
pub fn parse_http_date(s: &str) -> Result<SystemTime, Error> {
    s.parse::<HttpDate>().map(|d| d.into())
}
pub fn fmt_http_date(d: SystemTime) -> String {
    format!("{}", HttpDate::from(d))
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/httpdate.rs"]
mod tests;

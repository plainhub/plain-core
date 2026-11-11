use super::{ErrorKind, InvalidUri};
use std::fmt;
pub struct Port<T> {
    port: u16,
    repr: T,
}
impl<T> Port<T> {
    pub const fn as_u16(&self) -> u16 {
        self.port
    }
}
impl<T> Port<T>
where
    T: AsRef<str>,
{
    pub(crate) fn from_str(bytes: T) -> Result<Self, InvalidUri> {
        bytes
            .as_ref()
            .parse::<u16>()
            .map(|port| Port { port, repr: bytes })
            .map_err(|_| ErrorKind::InvalidPort.into())
    }
    pub fn as_str(&self) -> &str {
        self.repr.as_ref()
    }
}
impl<T> fmt::Debug for Port<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Port").field(&self.port).finish()
    }
}
impl<T> fmt::Display for Port<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.port, f)
    }
}
impl<T> From<Port<T>> for u16 {
    fn from(port: Port<T>) -> Self {
        port.as_u16()
    }
}
impl<T> AsRef<str> for Port<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl<T, U> PartialEq<Port<U>> for Port<T> {
    fn eq(&self, other: &Port<U>) -> bool {
        self.port == other.port
    }
}
impl<T> PartialEq<u16> for Port<T> {
    fn eq(&self, other: &u16) -> bool {
        self.port == *other
    }
}
impl<T> PartialEq<Port<T>> for u16 {
    fn eq(&self, other: &Port<T>) -> bool {
        other.port == *self
    }
}

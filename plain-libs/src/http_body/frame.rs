use crate::http::HeaderMap;
#[derive(Debug)]
pub struct Frame<T> {
    kind: Kind<T>,
}
#[derive(Debug)]
enum Kind<T> {
    Data(T),
    Trailers(HeaderMap),
}
impl<T> Frame<T> {
    pub fn data(buf: T) -> Self {
        Self {
            kind: Kind::Data(buf),
        }
    }
    pub fn trailers(map: HeaderMap) -> Self {
        Self {
            kind: Kind::Trailers(map),
        }
    }

    pub fn is_data(&self) -> bool {
        matches!(self.kind, Kind::Data(..))
    }
    pub fn into_data(self) -> Result<T, Self> {
        match self.kind {
            Kind::Data(data) => Ok(data),
            _ => Err(self),
        }
    }
    pub fn data_ref(&self) -> Option<&T> {
        match self.kind {
            Kind::Data(ref data) => Some(data),
            _ => None,
        }
    }

    pub fn is_trailers(&self) -> bool {
        matches!(self.kind, Kind::Trailers(..))
    }
    pub fn into_trailers(self) -> Result<HeaderMap, Self> {
        match self.kind {
            Kind::Trailers(trailers) => Ok(trailers),
            _ => Err(self),
        }
    }
}

#[derive(Debug)]
pub enum DomainError {
    Code(Box<dyn std::error::Error + Send + Sync>),
    Sqlite(plain_libs::rusqlite::Error),
    Other(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::Code(code) => write!(f, "{code}"),
            DomainError::Sqlite(e) => write!(f, "sqlite: {e}"),
            DomainError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DomainError::Code(code) => Some(code.as_ref()),
            DomainError::Sqlite(e) => Some(e),
            DomainError::Other(_) => None,
        }
    }
}

impl From<plain_libs::rusqlite::Error> for DomainError {
    fn from(e: plain_libs::rusqlite::Error) -> Self {
        DomainError::Sqlite(e)
    }
}

pub type DomainResult<T> = std::result::Result<T, DomainError>;

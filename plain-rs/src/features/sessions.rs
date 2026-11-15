use plain_libs::anyhow::{Result, anyhow};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum SessionType {
    Login,
    ApiToken,
}

impl plain_libs::rusqlite::types::ToSql for SessionType {
    fn to_sql(&self) -> plain_libs::rusqlite::Result<plain_libs::rusqlite::types::ToSqlOutput<'_>> {
        Ok(plain_libs::rusqlite::types::ToSqlOutput::Borrowed(
            plain_libs::rusqlite::types::ValueRef::Text(match self {
                Self::Login => b"LOGIN",
                Self::ApiToken => b"API_TOKEN",
            }),
        ))
    }
}
impl plain_libs::rusqlite::types::FromSql for SessionType {
    fn column_result(value: plain_libs::rusqlite::types::ValueRef<'_>) -> plain_libs::rusqlite::types::FromSqlResult<Self> {
        match value.as_str()? {
            "LOGIN" => Ok(Self::Login),
            "API_TOKEN" => Ok(Self::ApiToken),
            _ => Err(plain_libs::rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

pub fn token_key(token: &str) -> Result<[u8; 32]> {
    plain_libs::utils::base64::base64_decode_checked(token)
        .map_err(|_| anyhow!("invalid base64 token"))?
        .try_into()
        .map_err(|_| anyhow!("session token must contain 32 bytes"))
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/sessions.rs"]
mod tests;

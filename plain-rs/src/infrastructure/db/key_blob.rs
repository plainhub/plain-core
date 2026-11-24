use plain_libs::rusqlite::{Error, Result, Row};
use plain_libs::utils::base64::{base64_decode_checked, base64_encode};

pub(crate) fn encode(value: &str) -> Result<[u8; 32]> {
    base64_decode_checked(value)
        .map_err(|_| Error::ToSqlConversionFailure("invalid Base64 key".into()))?
        .try_into()
        .map_err(|_| Error::ToSqlConversionFailure("key must be 32 bytes".into()))
}

pub(crate) fn optional(value: &str) -> Result<Option<[u8; 32]>> {
    (!value.is_empty()).then(|| encode(value)).transpose()
}

pub(crate) fn read(row: &Row<'_>, column: usize) -> Result<String> {
    row.get::<_, [u8; 32]>(column).map(|key| base64_encode(&key))
}

pub(crate) fn read_optional(row: &Row<'_>, column: usize) -> Result<String> {
    row.get::<_, Option<[u8; 32]>>(column)
        .map(|key| key.map(|key| base64_encode(&key)).unwrap_or_default())
}

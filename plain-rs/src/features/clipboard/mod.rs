pub mod sync;

use crate::infrastructure::db::ClipboardRow;
use crate::infrastructure::db::Db;
use sha2::{Digest, Sha256};

pub fn record(
    db: &Db,
    text: &str,
    source: &str,
    label: &str,
    sensitive: bool,
) -> Result<(ClipboardRow, bool), String> {
    validate_text(text)?;
    let row = ClipboardRow {
        id: uuid::Uuid::new_v4().to_string(),
        hash: plain_libs::utils::hex::bytes_to_hex(&Sha256::digest(text.as_bytes())),
        text: text.into(),
        source: source.into(),
        label: label.into(),
        sensitive,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    db.clipboard_record(&row).map_err(|error| error.to_string())
}

pub fn validate_text(text: &str) -> Result<(), String> {
    if text.chars().all(char::is_whitespace) || text.encode_utf16().count() > 256 * 1024 {
        return Err("invalid_clipboard_text".into());
    }
    Ok(())
}

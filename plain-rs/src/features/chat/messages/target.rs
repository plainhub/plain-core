use anyhow::{Result, ensure};

pub fn parse(encoded: &str) -> Result<(String, String)> {
    if let Some(id) = encoded.strip_prefix("channel:") {
        ensure!(!id.is_empty(), "Missing channel target");
        Ok((String::new(), id.into()))
    } else {
        let id = encoded.strip_prefix("peer:").unwrap_or(encoded);
        ensure!(!id.is_empty(), "Missing peer target");
        Ok((id.into(), String::new()))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/target.rs"]
mod tests;

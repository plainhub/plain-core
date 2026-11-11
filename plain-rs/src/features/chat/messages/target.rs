use plain_libs::anyhow::{Result, bail, ensure};

pub fn parse(encoded: &str) -> Result<(String, String)> {
    if let Some(id) = encoded.strip_prefix("channel:") {
        ensure!(!id.is_empty(), "Missing channel target");
        Ok((String::new(), id.into()))
    } else if let Some(id) = encoded.strip_prefix("peer:") {
        ensure!(!id.is_empty(), "Missing peer target");
        Ok((id.into(), String::new()))
    } else {
        bail!("Target must use peer:<id> or channel:<id>")
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/target.rs"]
mod tests;

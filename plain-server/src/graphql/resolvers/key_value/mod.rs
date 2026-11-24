pub mod mutation;
pub mod query;

use plain_graphql::Result;

fn validate_key(key: &str) -> Result<()> {
    if matches!(
        key,
        "last_file_path" | "quick_note_draft" | "recent_searches" | "recent_save_dirs"
    ) || key.strip_prefix("chat_input_text:").is_some_and(|target| {
        (target.starts_with("channel:") || target.starts_with("peer:"))
            && target.split_once(':').is_some_and(|(_, id)| !id.is_empty())
    }) {
        Ok(())
    } else {
        Err(plain_graphql::Error::new("Unknown key-value key"))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/key_value/mod.rs"]
mod tests;

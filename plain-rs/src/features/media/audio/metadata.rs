pub fn normalize_artist(value: &str) -> String {
    value.replace("<unknown>", "")
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/audio/metadata.rs"]
mod tests;

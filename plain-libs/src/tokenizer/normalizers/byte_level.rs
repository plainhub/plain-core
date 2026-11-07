use crate::tokenizer::processors::byte_level::bytes_char;
use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use crate::tokenizer::utils::macro_rules_attribute;
use ahash::{AHashMap, AHashSet};
use std::sync::LazyLock;

#[derive(Clone, Debug)]
#[macro_rules_attribute(impl_serde_type!)]
pub struct ByteLevel;

static BYTES_CHAR: LazyLock<AHashMap<u8, char>> = LazyLock::new(bytes_char);

impl Default for ByteLevel {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteLevel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn alphabet() -> AHashSet<char> {
        BYTES_CHAR.values().copied().collect()
    }
}

impl Normalizer for ByteLevel {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        if !normalized.is_empty() {
            let s = normalized.get();
            let mut transformations: Vec<(char, isize)> = Vec::with_capacity(s.len());
            for (i, cur_char) in s.char_indices() {
                let size = cur_char.len_utf8();
                transformations.extend(
                    s.as_bytes()[i..i + size]
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (BYTES_CHAR[b], isize::from(i > 0))),
                );
            }
            normalized.transform(transformations, 0);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/byte_level/tests.rs"]
mod tests;

use crate::tokenizer::pre_tokenizers::byte_level::{
    ByteLevel as ByteLevelPreTokenizer, encode_bytes,
};
use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use std::collections::HashSet;

impl_serde_type! {
    #[derive(Clone, Debug)]
    pub struct ByteLevel;
}

impl Default for ByteLevel {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteLevel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn alphabet() -> HashSet<char> {
        ByteLevelPreTokenizer::alphabet()
    }
}

impl Normalizer for ByteLevel {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        if !normalized.is_empty() {
            return encode_bytes(normalized);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/byte_level/tests.rs"]
mod tests;

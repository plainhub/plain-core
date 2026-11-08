use crate::tokenizer::normalizer::Range;
use crate::tokenizer::tokenizer::{PreTokenizedString, PreTokenizer, Result};
use serde::{Deserialize, Serialize};

impl_serde_type! {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FixedLength {
        #[serde(default = "default_length")]
        pub length: usize,
    }
}

impl FixedLength {
    pub fn new(length: usize) -> Self {
        Self { length }
    }
}

fn default_length() -> usize {
    5
}

impl PreTokenizer for FixedLength {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, normalized| {
            let text = normalized.get();
            if text.is_empty() {
                return Ok(vec![]);
            }

            let mut splits = Vec::new();
            let char_positions: Vec<_> = text.char_indices().collect();
            for chunk in char_positions.chunks(self.length) {
                let start = chunk.first().map(|(i, _)| *i).unwrap_or(0);
                let end = chunk
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(text.len());
                splits.push(
                    normalized
                        .slice(Range::Normalized(start..end))
                        .ok_or("Failed to slice normalized text")?,
                );
            }

            Ok(splits)
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/fixed_length/tests.rs"]
mod tests;

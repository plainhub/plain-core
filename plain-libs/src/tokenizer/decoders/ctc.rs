use crate::tokenizer::decoders::wordpiece;
use crate::tokenizer::tokenizer::{Decoder, Result};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub struct CTC {
    pub pad_token: String,

    pub word_delimiter_token: String,

    pub cleanup: bool,
}

impl CTC {
    pub fn new(pad_token: String, word_delimiter_token: String, cleanup: bool) -> Self {
        Self {
            pad_token,
            word_delimiter_token,
            cleanup,
        }
    }
}

impl Default for CTC {
    fn default() -> Self {
        Self {
            pad_token: "<pad>".to_string(),
            word_delimiter_token: "|".to_string(),
            cleanup: true,
        }
    }
}

impl Decoder for CTC {
    fn decode_chain(&self, mut tokens: Vec<String>) -> Result<Vec<String>> {
        tokens.dedup();
        Ok(tokens
            .into_iter()
            .filter_map(|token| {
                let mut replaced = token.replace(&self.pad_token, "");
                if self.cleanup {
                    replaced =
                        wordpiece::cleanup(&replaced).replace(&self.word_delimiter_token, " ");
                }
                if replaced.is_empty() {
                    None
                } else {
                    Some(replaced)
                }
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/ctc/tests.rs"]
mod tests;

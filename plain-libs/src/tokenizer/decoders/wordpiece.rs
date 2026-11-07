use crate::tokenizer::tokenizer::{Decoder, Result};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Serialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub struct WordPiece {
    pub prefix: String,

    pub cleanup: bool,
}

impl WordPiece {
    pub fn new(prefix: String, cleanup: bool) -> Self {
        Self { prefix, cleanup }
    }
}

impl Default for WordPiece {
    fn default() -> Self {
        Self {
            prefix: "##".to_owned(),
            cleanup: true,
        }
    }
}
pub fn cleanup(dirty_input: &str) -> String {
    dirty_input
        .replace(" .", ".")
        .replace(" ?", "?")
        .replace(" !", "!")
        .replace(" ,", ",")
        .replace(" ' ", "'")
        .replace(" n't", "n't")
        .replace(" 'm", "'m")
        .replace(" do not", " don't")
        .replace(" 's", "'s")
        .replace(" 've", "'ve")
        .replace(" 're", "'re")
}

impl Decoder for WordPiece {
    fn decode_chain(&self, mut tokens: Vec<String>) -> Result<Vec<String>> {
        for (i, token) in tokens.iter_mut().enumerate() {
            if i != 0 {
                if let Some(tk) = token.strip_prefix(&self.prefix) {
                    *token = tk.to_string();
                } else {
                    *token = format!(" {token}");
                }
            }
            if self.cleanup {
                *token = cleanup(token);
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/wordpiece/tests.rs"]
mod tests;

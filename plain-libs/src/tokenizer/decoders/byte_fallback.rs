use crate::tokenizer::tokenizer::{Decoder, Result};
use monostate::MustBe;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug, Serialize, Default)]
#[non_exhaustive]
pub struct ByteFallback {
    #[serde(rename = "type")]
    type_: MustBe!("ByteFallback"),
}

impl ByteFallback {
    pub fn new() -> Self {
        Self {
            type_: MustBe!("ByteFallback"),
        }
    }
}

impl Decoder for ByteFallback {
    fn decode_chain(&self, tokens: Vec<String>) -> Result<Vec<String>> {
        let mut new_tokens: Vec<String> = vec![];
        let mut previous_byte_tokens: Vec<u8> = vec![];

        for token in tokens {
            let bytes = if token.len() == 6 && token.starts_with("<0x") && token.ends_with('>') {
                u8::from_str_radix(&token[3..5], 16).ok()
            } else {
                None
            };
            if let Some(bytes) = bytes {
                previous_byte_tokens.push(bytes);
            } else {
                if !previous_byte_tokens.is_empty() {
                    if let Ok(string) = String::from_utf8(previous_byte_tokens.clone()) {
                        new_tokens.push(string);
                    } else {
                        for _ in 0..previous_byte_tokens.len() {
                            new_tokens.push("�".into());
                        }
                    }
                    previous_byte_tokens.clear();
                }
                new_tokens.push(token);
            }
        }
        if !previous_byte_tokens.is_empty() {
            if let Ok(string) = String::from_utf8(previous_byte_tokens.clone()) {
                new_tokens.push(string);
            } else {
                for _ in 0..previous_byte_tokens.len() {
                    new_tokens.push("�".into());
                }
            }
        }

        Ok(new_tokens)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/byte_fallback/tests.rs"]
mod tests;

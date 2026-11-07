use crate::tokenizer::tokenizer::{Decoder, Result};
use monostate::MustBe;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Fuse {
    #[serde(rename = "type")]
    type_: MustBe!("Fuse"),
}

impl Fuse {
    pub fn new() -> Self {
        Self {
            type_: MustBe!("Fuse"),
        }
    }
}

impl Decoder for Fuse {
    fn decode_chain(&self, tokens: Vec<String>) -> Result<Vec<String>> {
        let new_string = tokens.join("");
        Ok(vec![new_string])
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/fuse/tests.rs"]
mod tests;

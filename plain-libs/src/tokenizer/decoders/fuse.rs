use crate::tokenizer::tokenizer::{Decoder, Result};

impl_serde_type! {
    #[derive(Clone, Debug, Default)]
    #[non_exhaustive]
    pub struct Fuse;
}

impl Fuse {
    pub fn new() -> Self {
        Self
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

use crate::tokenizer::decoders::DecoderWrapper;
use crate::tokenizer::tokenizer::{Decoder, Result};
use serde::{Deserialize, Serialize};

impl_serde_type! {
    #[derive(Clone, Debug)]
    pub struct Sequence {
        decoders: Vec<DecoderWrapper>,
    }
}

impl Sequence {
    pub fn new(decoders: Vec<DecoderWrapper>) -> Self {
        Self { decoders }
    }

    pub fn get_decoders(&self) -> &[DecoderWrapper] {
        &self.decoders
    }

    pub fn get_decoders_mut(&mut self) -> &mut [DecoderWrapper] {
        &mut self.decoders
    }
}

impl Decoder for Sequence {
    fn decode_chain(&self, mut tokens: Vec<String>) -> Result<Vec<String>> {
        for decoder in &self.decoders {
            tokens = decoder.decode_chain(tokens)?;
        }
        Ok(tokens)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/sequence/tests.rs"]
mod tests;

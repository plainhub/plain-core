use crate::tokenizer::tokenizer::{Decoder, Result};

impl_serde_type! {
    #[derive(Clone, Debug, Default)]
    #[non_exhaustive]
    pub struct ByteFallback;
}

impl ByteFallback {
    pub fn new() -> Self {
        Self
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
                flush_bytes(&mut previous_byte_tokens, &mut new_tokens);
                new_tokens.push(token);
            }
        }
        flush_bytes(&mut previous_byte_tokens, &mut new_tokens);

        Ok(new_tokens)
    }
}

fn flush_bytes(bytes: &mut Vec<u8>, tokens: &mut Vec<String>) {
    if bytes.is_empty() {
        return;
    }
    match String::from_utf8(std::mem::take(bytes)) {
        Ok(text) => tokens.push(text),
        Err(error) => tokens.resize_with(tokens.len() + error.as_bytes().len(), || "�".into()),
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/byte_fallback/tests.rs"]
mod tests;

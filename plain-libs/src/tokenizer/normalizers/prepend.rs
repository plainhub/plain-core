use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub struct Prepend {
    pub prepend: String,
}

impl Prepend {
    pub fn new(prepend: String) -> Self {
        Self { prepend }
    }
}

impl Normalizer for Prepend {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        if !normalized.is_empty() {
            normalized.prepend(&self.prepend);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/prepend/tests.rs"]
mod tests;

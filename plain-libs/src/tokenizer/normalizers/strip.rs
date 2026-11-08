use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use serde::{Deserialize, Serialize};
use unicode_normalization_alignments::char::is_combining_mark;

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub struct Strip {
    pub strip_left: bool,
    pub strip_right: bool,
}

impl Strip {
    pub fn new(strip_left: bool, strip_right: bool) -> Self {
        Self {
            strip_left,
            strip_right,
        }
    }
}

impl Normalizer for Strip {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        if self.strip_left && self.strip_right {
            // Fast path
            normalized.strip();
        } else {
            if self.strip_left {
                normalized.lstrip();
            }

            if self.strip_right {
                normalized.rstrip();
            }
        }

        Ok(())
    }
}

// This normalizer removes combining marks from a normalized string
// It's different from unidecode as it does not attempt to modify
// non ascii languages.
impl_serde_type! {
    #[derive(Copy, Clone, Debug)]
    pub struct StripAccents;
}

impl Normalizer for StripAccents {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        normalized.filter(|c| !is_combining_mark(c));
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/strip/tests.rs"]
mod tests;

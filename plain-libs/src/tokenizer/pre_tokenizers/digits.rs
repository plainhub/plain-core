use serde::{Deserialize, Serialize};

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use crate::tokenizer::utils::macro_rules_attribute;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[macro_rules_attribute(impl_serde_type!)]
pub struct Digits {
    pub individual_digits: bool,
}

impl Digits {
    pub fn new(individual_digits: bool) -> Self {
        Self { individual_digits }
    }
}

impl Default for Digits {
    fn default() -> Self {
        Self::new(false)
    }
}

impl PreTokenizer for Digits {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        if self.individual_digits {
            pretokenized.split(|_, normalized| {
                normalized.split(char::is_numeric, SplitDelimiterBehavior::Isolated)
            })
        } else {
            pretokenized.split(|_, normalized| {
                normalized.split(char::is_numeric, SplitDelimiterBehavior::Contiguous)
            })
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/digits/tests.rs"]
mod tests;

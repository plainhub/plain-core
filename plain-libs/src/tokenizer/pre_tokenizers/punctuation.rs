use serde::{Deserialize, Serialize};

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use crate::tokenizer::utils::macro_rules_attribute;
use unicode_categories::UnicodeCategories;

fn is_punc(x: char) -> bool {
    char::is_ascii_punctuation(&x) || x.is_punctuation()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[macro_rules_attribute(impl_serde_type!)]
pub struct Punctuation {
    #[serde(default = "default_split")]
    pub behavior: SplitDelimiterBehavior,
}

fn default_split() -> SplitDelimiterBehavior {
    SplitDelimiterBehavior::Isolated
}

impl Punctuation {
    pub fn new(behavior: SplitDelimiterBehavior) -> Self {
        Self { behavior }
    }
}

impl Default for Punctuation {
    fn default() -> Self {
        Self::new(SplitDelimiterBehavior::Isolated)
    }
}

impl PreTokenizer for Punctuation {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, s| s.split(is_punc, self.behavior))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/punctuation/tests.rs"]
mod tests;

use serde::{Deserialize, Serialize};

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use crate::tokenizer::unicode_categories::is_punctuation;

impl_serde_type! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct Punctuation {
        #[serde(default = "default_split")]
        pub behavior: SplitDelimiterBehavior,
    }
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
        pretokenized.split(|_, s| s.split(is_punctuation, self.behavior))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/punctuation/tests.rs"]
mod tests;

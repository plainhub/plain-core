use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use crate::tokenizer::unicode_categories::is_punctuation;

impl_serde_type! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct BertPreTokenizer;
}

impl PreTokenizer for BertPreTokenizer {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, s| s.split(char::is_whitespace, SplitDelimiterBehavior::Removed))?;
        pretokenized.split(|_, s| s.split(is_punctuation, SplitDelimiterBehavior::Isolated))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/bert/tests.rs"]
mod tests;

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior,
};
use crate::tokenizer::utils::macro_rules_attribute;
use unicode_categories::UnicodeCategories;

fn is_bert_punc(x: char) -> bool {
    char::is_ascii_punctuation(&x) || x.is_punctuation()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[macro_rules_attribute(impl_serde_type!)]
pub struct BertPreTokenizer;

impl PreTokenizer for BertPreTokenizer {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, s| s.split(char::is_whitespace, SplitDelimiterBehavior::Removed))?;
        pretokenized.split(|_, s| s.split(is_bert_punc, SplitDelimiterBehavior::Isolated))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/bert/tests.rs"]
mod tests;

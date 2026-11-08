use std::sync::LazyLock;

use crate::regex::{self, Regex};

use crate::tokenizer::tokenizer::{
    PreTokenizedString, PreTokenizer, Result, SplitDelimiterBehavior, pattern::Invert,
};

impl_serde_type! {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Whitespace;
}

impl Default for Whitespace {
    fn default() -> Self {
        Self
    }
}

impl PreTokenizer for Whitespace {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        static RE: LazyLock<Regex> = LazyLock::new(|| regex::compile(r"\w+|[^\w\s]+").unwrap());
        let re_ref: &Regex = &RE;

        pretokenized.split(|_, normalized| {
            normalized.split(Invert(re_ref), SplitDelimiterBehavior::Removed)
        })
    }
}

impl_serde_type! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct WhitespaceSplit;
}

impl PreTokenizer for WhitespaceSplit {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        pretokenized.split(|_, normalized| {
            normalized.split(char::is_whitespace, SplitDelimiterBehavior::Removed)
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/whitespace/tests.rs"]
mod tests;

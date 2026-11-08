use crate::tokenizer::pre_tokenizers::PreTokenizerWrapper;
use crate::tokenizer::tokenizer::{PreTokenizedString, PreTokenizer, Result};
use serde::{Deserialize, Serialize};

impl_serde_type! {
    #[derive(Clone, Debug, PartialEq)]
    pub struct Sequence {
        pretokenizers: Vec<PreTokenizerWrapper>,
    }
}

impl Sequence {
    pub fn new(pretokenizers: Vec<PreTokenizerWrapper>) -> Self {
        Self { pretokenizers }
    }
}

impl AsRef<[PreTokenizerWrapper]> for Sequence {
    fn as_ref(&self) -> &[PreTokenizerWrapper] {
        &self.pretokenizers
    }
}

impl AsMut<[PreTokenizerWrapper]> for Sequence {
    fn as_mut(&mut self) -> &mut [PreTokenizerWrapper] {
        &mut self.pretokenizers
    }
}

impl IntoIterator for Sequence {
    type Item = PreTokenizerWrapper;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.pretokenizers.into_iter()
    }
}

impl PreTokenizer for Sequence {
    fn pre_tokenize(&self, pretokenized: &mut PreTokenizedString) -> Result<()> {
        for pretokenizer in &self.pretokenizers {
            pretokenizer.pre_tokenize(pretokenized)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/sequence/tests.rs"]
mod tests;

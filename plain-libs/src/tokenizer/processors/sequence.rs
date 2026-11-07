use crate::tokenizer::processors::PostProcessorWrapper;
use crate::tokenizer::tokenizer::{Encoding, PostProcessor, Result};
use crate::tokenizer::utils::macro_rules_attribute;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
#[macro_rules_attribute(impl_serde_type!)]
pub struct Sequence {
    processors: Vec<PostProcessorWrapper>,
}

impl Sequence {
    pub fn new(processors: Vec<PostProcessorWrapper>) -> Self {
        Self { processors }
    }

    pub fn get(&self, index: usize) -> Option<&PostProcessorWrapper> {
        self.processors.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut PostProcessorWrapper> {
        self.processors.get_mut(index)
    }

    pub fn set_mut(&mut self, index: usize, post_proc: PostProcessorWrapper) {
        self.processors[index] = post_proc;
    }
}

impl AsRef<[PostProcessorWrapper]> for Sequence {
    fn as_ref(&self) -> &[PostProcessorWrapper] {
        &self.processors
    }
}

impl AsMut<[PostProcessorWrapper]> for Sequence {
    fn as_mut(&mut self) -> &mut [PostProcessorWrapper] {
        &mut self.processors
    }
}

impl IntoIterator for Sequence {
    type Item = PostProcessorWrapper;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.processors.into_iter()
    }
}

impl PostProcessor for Sequence {
    fn added_tokens(&self, is_pair: bool) -> usize {
        self.processors
            .iter()
            .map(|p| p.added_tokens(is_pair))
            .sum::<usize>()
    }

    fn process_encodings(
        &self,
        mut encodings: Vec<Encoding>,
        add_special_tokens: bool,
    ) -> Result<Vec<Encoding>> {
        for processor in &self.processors {
            encodings = processor.process_encodings(encodings, add_special_tokens)?;
        }
        Ok(encodings)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/processors/sequence/tests.rs"]
mod tests;

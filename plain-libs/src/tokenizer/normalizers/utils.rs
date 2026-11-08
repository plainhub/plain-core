use serde::{Deserialize, Serialize};

use crate::tokenizer::normalizers::NormalizerWrapper;
use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};

#[derive(Clone, Deserialize, Debug, Serialize)]
#[serde(tag = "type")]

pub struct Sequence {
    normalizers: Vec<NormalizerWrapper>,
}

impl Sequence {
    pub fn new(normalizers: Vec<NormalizerWrapper>) -> Self {
        Self { normalizers }
    }
}

impl AsRef<[NormalizerWrapper]> for Sequence {
    fn as_ref(&self) -> &[NormalizerWrapper] {
        &self.normalizers
    }
}

impl AsMut<[NormalizerWrapper]> for Sequence {
    fn as_mut(&mut self) -> &mut [NormalizerWrapper] {
        &mut self.normalizers
    }
}

impl IntoIterator for Sequence {
    type Item = NormalizerWrapper;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.normalizers.into_iter()
    }
}

impl Normalizer for Sequence {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        for normalizer in &self.normalizers {
            normalizer.normalize(normalized)?;
        }
        Ok(())
    }
}

impl_serde_type! {
    #[derive(Copy, Clone, Debug)]
    pub struct Lowercase;
}
impl Normalizer for Lowercase {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        normalized.lowercase();
        Ok(())
    }
}

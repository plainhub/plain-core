pub mod bert;
pub mod roberta;
pub mod sequence;
pub mod template;

// Re-export these as processors
pub use super::pre_tokenizers::byte_level;

use serde::{Deserialize, Serialize};

use crate::tokenizer::pre_tokenizers::byte_level::ByteLevel;
use crate::tokenizer::processors::bert::BertProcessing;
use crate::tokenizer::processors::roberta::RobertaProcessing;
use crate::tokenizer::processors::sequence::Sequence;
use crate::tokenizer::processors::template::TemplateProcessing;
use crate::tokenizer::{Encoding, PostProcessor, Result};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Eq)]
#[serde(untagged)]
pub enum PostProcessorWrapper {
    // Roberta must be before Bert for deserialization (serde does not validate tags)
    Roberta(RobertaProcessing),
    Bert(BertProcessing),
    ByteLevel(ByteLevel),
    Template(TemplateProcessing),
    Sequence(Sequence),
}

impl PostProcessor for PostProcessorWrapper {
    fn added_tokens(&self, is_pair: bool) -> usize {
        match self {
            Self::Bert(bert) => bert.added_tokens(is_pair),
            Self::ByteLevel(bl) => bl.added_tokens(is_pair),
            Self::Roberta(roberta) => roberta.added_tokens(is_pair),
            Self::Template(template) => template.added_tokens(is_pair),
            Self::Sequence(bl) => bl.added_tokens(is_pair),
        }
    }

    fn process_encodings(
        &self,
        encodings: Vec<Encoding>,
        add_special_tokens: bool,
    ) -> Result<Vec<Encoding>> {
        match self {
            Self::Bert(bert) => bert.process_encodings(encodings, add_special_tokens),
            Self::ByteLevel(bl) => bl.process_encodings(encodings, add_special_tokens),
            Self::Roberta(roberta) => roberta.process_encodings(encodings, add_special_tokens),
            Self::Template(template) => template.process_encodings(encodings, add_special_tokens),
            Self::Sequence(bl) => bl.process_encodings(encodings, add_special_tokens),
        }
    }
}

impl_enum_from!(BertProcessing, PostProcessorWrapper, Bert);
impl_enum_from!(ByteLevel, PostProcessorWrapper, ByteLevel);
impl_enum_from!(RobertaProcessing, PostProcessorWrapper, Roberta);
impl_enum_from!(TemplateProcessing, PostProcessorWrapper, Template);
impl_enum_from!(Sequence, PostProcessorWrapper, Sequence);

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/processors/mod/tests.rs"]
mod tests;

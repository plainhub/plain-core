pub mod bpe;
pub mod unigram;
pub mod wordlevel;
pub mod wordpiece;

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::tokenizer::models::bpe::BPE;
use crate::tokenizer::models::unigram::Unigram;
use crate::tokenizer::models::wordlevel::WordLevel;
use crate::tokenizer::models::wordpiece::WordPiece;
use crate::tokenizer::{Model, Result, Token};

struct OrderedVocabIter<'a> {
    vocab_r: &'a HashMap<u32, String>,
}

impl<'a> OrderedVocabIter<'a> {
    fn new(vocab_r: &'a HashMap<u32, String>) -> Self {
        Self { vocab_r }
    }
}

impl Serialize for OrderedVocabIter<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // There could be holes so max + 1 is more correct than vocab_r.len()
        let mut holes = vec![];
        let result = if let Some(max) = self.vocab_r.keys().max() {
            let iter = (0..*max + 1).filter_map(|i| {
                if let Some(token) = self.vocab_r.get(&i) {
                    Some((token, i))
                } else {
                    holes.push(i);
                    None
                }
            });
            serializer.collect_map(iter)
        } else {
            serializer.collect_map(std::iter::empty::<(&str, u32)>())
        };

        if !holes.is_empty() {
            log::warn!(
                "The OrderedVocab you are attempting to save contains holes for indices {holes:?}, your vocabulary could be corrupted!"
            );
            println!(
                "The OrderedVocab you are attempting to save contains holes for indices {holes:?}, your vocabulary could be corrupted!"
            );
        }
        result
    }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum ModelWrapper {
    BPE(BPE),
    // WordPiece must stay before WordLevel here for deserialization (for retrocompatibility
    // with the versions not including the "type"), since WordLevel is a subset of WordPiece
    WordPiece(WordPiece),
    WordLevel(WordLevel),
    Unigram(Unigram),
}

impl<'de> Deserialize<'de> for ModelWrapper {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        pub struct Tagged {
            #[serde(rename = "type")]
            variant: EnumType,
            #[serde(flatten)]
            rest: serde_json::Value,
        }
        #[derive(Deserialize)]
        pub enum EnumType {
            BPE,
            WordPiece,
            WordLevel,
            Unigram,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum ModelHelper {
            Tagged(Tagged),
            Legacy(serde_json::Value),
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum ModelUntagged {
            BPE(BPE),
            // WordPiece must stay before WordLevel here for deserialization (for retrocompatibility
            // with the versions not including the "type"), since WordLevel is a subset of WordPiece
            WordPiece(WordPiece),
            WordLevel(WordLevel),
            Unigram(Unigram),
        }

        let helper = ModelHelper::deserialize(deserializer)?;
        Ok(match helper {
            ModelHelper::Tagged(model) => match model.variant {
                EnumType::BPE => ModelWrapper::BPE(
                    serde_json::from_value(model.rest).map_err(serde::de::Error::custom)?,
                ),
                EnumType::WordPiece => ModelWrapper::WordPiece(
                    serde_json::from_value(model.rest).map_err(serde::de::Error::custom)?,
                ),
                EnumType::WordLevel => ModelWrapper::WordLevel(
                    serde_json::from_value(model.rest).map_err(serde::de::Error::custom)?,
                ),
                EnumType::Unigram => ModelWrapper::Unigram(
                    serde_json::from_value(model.rest).map_err(serde::de::Error::custom)?,
                ),
            },
            ModelHelper::Legacy(value) => {
                let untagged = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                match untagged {
                    ModelUntagged::BPE(bpe) => ModelWrapper::BPE(bpe),
                    ModelUntagged::WordPiece(bpe) => ModelWrapper::WordPiece(bpe),
                    ModelUntagged::WordLevel(bpe) => ModelWrapper::WordLevel(bpe),
                    ModelUntagged::Unigram(bpe) => ModelWrapper::Unigram(bpe),
                }
            }
        })
    }
}

impl_enum_from!(WordLevel, ModelWrapper, WordLevel);
impl_enum_from!(WordPiece, ModelWrapper, WordPiece);
impl_enum_from!(BPE, ModelWrapper, BPE);
impl_enum_from!(Unigram, ModelWrapper, Unigram);

impl Model for ModelWrapper {
    fn tokenize(&self, tokens: &str) -> Result<Vec<Token>> {
        match self {
            Self::WordLevel(t) => t.tokenize(tokens),
            Self::WordPiece(t) => t.tokenize(tokens),
            Self::BPE(t) => t.tokenize(tokens),
            Self::Unigram(t) => t.tokenize(tokens),
        }
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        match self {
            Self::WordLevel(t) => t.token_to_id(token),
            Self::WordPiece(t) => t.token_to_id(token),
            Self::BPE(t) => t.token_to_id(token),
            Self::Unigram(t) => t.token_to_id(token),
        }
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        match self {
            Self::WordLevel(t) => t.id_to_token(id),
            Self::WordPiece(t) => t.id_to_token(id),
            Self::BPE(t) => t.id_to_token(id),
            Self::Unigram(t) => t.id_to_token(id),
        }
    }

    fn get_vocab(&self) -> HashMap<String, u32> {
        match self {
            Self::WordLevel(t) => t.get_vocab(),
            Self::WordPiece(t) => t.get_vocab(),
            Self::BPE(t) => t.get_vocab(),
            Self::Unigram(t) => t.get_vocab(),
        }
    }

    fn get_vocab_size(&self) -> usize {
        match self {
            Self::WordLevel(t) => t.get_vocab_size(),
            Self::WordPiece(t) => t.get_vocab_size(),
            Self::BPE(t) => t.get_vocab_size(),
            Self::Unigram(t) => t.get_vocab_size(),
        }
    }
}

impl ModelWrapper {
    pub fn clear_cache(&mut self) {
        match self {
            Self::Unigram(model) => model.clear_cache(),
            Self::BPE(model) => model.clear_cache(),
            _ => (),
        }
    }
    pub fn resize_cache(&mut self, capacity: usize) {
        match self {
            Self::Unigram(model) => model.resize_cache(capacity),
            Self::BPE(model) => model.resize_cache(capacity),
            _ => (),
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/models/mod/tests.rs"]
mod tests;

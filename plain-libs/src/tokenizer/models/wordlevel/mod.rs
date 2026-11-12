use super::OrderedVocabIter;
use crate::tokenizer::tokenizer::{Model, Result, Token};
use std::collections::HashMap;

mod serialization;

// Re-export

type Vocab = HashMap<String, u32>;

#[derive(Debug)]
pub enum Error {
    MissingUnkToken,
    BadVocabulary,
}

struct Config {
    vocab: HashMap<String, u32>,
    unk_token: String,
}

pub struct WordLevelBuilder {
    config: Config,
}

impl Default for WordLevelBuilder {
    fn default() -> Self {
        Self {
            config: Config {
                vocab: HashMap::new(),
                unk_token: String::from("<unk>"),
            },
        }
    }
}

impl WordLevelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn vocab(mut self, vocab: HashMap<String, u32>) -> Self {
        self.config.vocab = vocab;
        self
    }

    #[must_use]
    pub fn unk_token(mut self, unk_token: String) -> Self {
        self.config.unk_token = unk_token;
        self
    }

    pub fn build(mut self) -> Result<WordLevel> {
        let vocab_r = self
            .config
            .vocab
            .iter()
            .map(|(key, val)| (*val, key.to_owned()))
            .collect();

        Ok(WordLevel {
            vocab: self.config.vocab,
            vocab_r,
            unk_token: self.config.unk_token,
        })
    }
}

#[derive(PartialEq, Clone, Eq)]
pub struct WordLevel {
    vocab: HashMap<String, u32>,
    vocab_r: HashMap<u32, String>,
    pub unk_token: String,
}

impl std::fmt::Debug for WordLevel {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("WordLevel")
            .field("unk_token", &self.unk_token)
            .field("vocab", &self.vocab.len())
            .finish()
    }
}

impl WordLevel {
    pub fn builder() -> WordLevelBuilder {
        WordLevelBuilder::new()
    }
}

impl Default for WordLevel {
    fn default() -> Self {
        Self {
            vocab: HashMap::new(),
            vocab_r: HashMap::new(),
            unk_token: String::from("<unk>"),
        }
    }
}

impl Model for WordLevel {
    fn tokenize(&self, token: &str) -> Result<Vec<Token>> {
        if let Some(&id) = self.vocab.get(token) {
            Ok(vec![Token {
                id,
                value: token.to_owned(),
                offsets: (0, token.len()),
            }])
        } else if let Some(&unk_id) = self.vocab.get(&self.unk_token) {
            Ok(vec![Token {
                id: unk_id,
                value: self.unk_token.to_owned(),
                offsets: (0, token.len()),
            }])
        } else {
            Err(Box::new(Error::MissingUnkToken))
        }
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.vocab.get(token).copied()
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        self.vocab_r.get(&id).cloned()
    }

    fn get_vocab(&self) -> HashMap<String, u32> {
        self.vocab.clone().into_iter().collect()
    }

    fn get_vocab_size(&self) -> usize {
        self.vocab.keys().len()
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/wordlevel/mod/tests.rs"]
mod tests;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingUnkToken => {
                f.write_str("WordLevel error: Missing [UNK] token from the vocabulary")
            }
            Self::BadVocabulary => f.write_str("Bad vocabulary json file"),
        }
    }
}

impl std::error::Error for Error {}

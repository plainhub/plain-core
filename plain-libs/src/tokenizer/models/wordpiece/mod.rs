use crate::tokenizer::tokenizer::{Model, Result, Token};
use ahash::AHashMap;
use std::borrow::Cow;
use std::collections::HashMap;

mod serialization;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("WordPiece error: Missing [UNK] token from the vocabulary")]
    MissingUnkToken,
}

type Vocab = AHashMap<String, u32>;
type VocabR = AHashMap<u32, String>;

struct Config {
    vocab: Vocab,
    unk_token: String,
    continuing_subword_prefix: String,
    max_input_chars_per_word: usize,
}

pub struct WordPieceBuilder {
    config: Config,
}

impl Default for WordPieceBuilder {
    fn default() -> Self {
        Self {
            config: Config {
                vocab: AHashMap::new(),
                unk_token: String::from("[UNK]"),
                continuing_subword_prefix: String::from("##"),
                max_input_chars_per_word: 100,
            },
        }
    }
}

impl WordPieceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn vocab<V: Into<AHashMap<String, u32>>>(mut self, vocab: V) -> Self {
        self.config.vocab = vocab.into();
        self
    }

    #[must_use]
    pub fn unk_token(mut self, unk_token: String) -> Self {
        self.config.unk_token = unk_token;
        self
    }

    #[must_use]
    pub fn continuing_subword_prefix(mut self, continuing_subword_prefix: String) -> Self {
        self.config.continuing_subword_prefix = continuing_subword_prefix;
        self
    }

    #[must_use]
    pub fn max_input_chars_per_word(mut self, max_input_chars_per_word: usize) -> Self {
        self.config.max_input_chars_per_word = max_input_chars_per_word;
        self
    }

    pub fn build(mut self) -> Result<WordPiece> {
        let vocab_r = self
            .config
            .vocab
            .iter()
            .map(|(key, val)| (*val, key.to_owned()))
            .collect();

        Ok(WordPiece {
            vocab: self.config.vocab,
            vocab_r,
            unk_token: self.config.unk_token,
            continuing_subword_prefix: self.config.continuing_subword_prefix,
            max_input_chars_per_word: self.config.max_input_chars_per_word,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WordPiece {
    vocab: Vocab,
    vocab_r: VocabR,
    pub unk_token: String,
    pub continuing_subword_prefix: String,
    pub max_input_chars_per_word: usize,
}

impl std::fmt::Debug for WordPiece {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("WordPiece")
            .field("unk_token", &self.unk_token)
            .field("continuing_subword_prefix", &self.continuing_subword_prefix)
            .field("max_input_chars_per_word", &self.max_input_chars_per_word)
            .field("vocab", &self.vocab.len())
            .finish()
    }
}

impl Default for WordPiece {
    fn default() -> Self {
        Self {
            vocab: AHashMap::new(),
            vocab_r: AHashMap::new(),
            unk_token: String::from("[UNK]"),
            continuing_subword_prefix: String::from("##"),
            max_input_chars_per_word: 100,
        }
    }
}

impl WordPiece {
    pub fn builder() -> WordPieceBuilder {
        WordPieceBuilder::new()
    }

    pub fn from_bytes<P: AsRef<[u8]>>(bytes: P) -> Result<Self> {
        let tokenizer = serde_json::from_slice(bytes.as_ref())?;
        Ok(tokenizer)
    }
}

impl Model for WordPiece {
    fn get_vocab(&self) -> HashMap<String, u32> {
        self.vocab.clone().into_iter().collect()
    }

    fn get_vocab_size(&self) -> usize {
        self.vocab.len()
    }

    fn tokenize(&self, sequence: &str) -> Result<Vec<Token>> {
        let char_len = sequence.chars().count();
        if char_len > self.max_input_chars_per_word {
            return Ok(vec![Token {
                value: self.unk_token.clone(),
                id: *self
                    .vocab
                    .get(&self.unk_token)
                    .ok_or(Error::MissingUnkToken)?,
                offsets: (0, sequence.len()),
            }]);
        }

        let mut is_bad = false;
        let mut start = 0;
        let mut sub_tokens: Vec<Token> = vec![];

        while start < sequence.len() {
            let mut end = sequence.len();
            let mut cur_str = None;

            while start < end {
                let mut substr: Cow<str> = Cow::Borrowed(&sequence[start..end]);

                if start > 0 {
                    substr = Cow::Owned(format!("{}{}", self.continuing_subword_prefix, substr));
                }
                if self.vocab.contains_key(substr.as_ref()) {
                    cur_str = Some(Token {
                        id: self.vocab[substr.as_ref()],
                        value: substr.to_string(),
                        offsets: (start, end),
                    });
                    break;
                }
                end -= substr.chars().last().map_or(1, |c| c.len_utf8());
            }

            if cur_str.is_none() {
                is_bad = true;
                break;
            }

            sub_tokens.push(cur_str.unwrap());
            start = end;
        }

        if is_bad {
            Ok(vec![Token {
                value: self.unk_token.clone(),
                id: *self
                    .vocab
                    .get(&self.unk_token)
                    .ok_or(Error::MissingUnkToken)?,
                offsets: (0, sequence.len()),
            }])
        } else {
            Ok(sub_tokens)
        }
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.vocab.get(token).copied()
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        self.vocab_r.get(&id).cloned()
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/wordpiece/mod/tests.rs"]
mod tests;

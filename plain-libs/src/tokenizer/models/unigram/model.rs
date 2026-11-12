use super::{
    lattice::Lattice,
    trie::{Trie, TrieBuilder},
};
use crate::tokenizer::tokenizer::{Model, Result, Token};
use crate::tokenizer::utils::cache::{Cache, MAX_LENGTH};
use std::collections::HashMap;

use std::convert::TryInto;

type TokenMap = HashMap<String, u32>;
type Vocab = Vec<(String, f64)>;

pub struct Unigram {
    token_to_ids: TokenMap,
    pub(crate) vocab: Vocab,
    cache: Cache<String, Vec<String>>,
    trie: Trie<u8>,
    pub min_score: f64,
    pub(super) unk_id: Option<usize>,
    pub(super) bos_id: usize,
    pub(super) eos_id: usize,

    fuse_unk: bool,
    is_optimized: bool,
    byte_fallback: bool,
}
impl PartialEq for Unigram {
    fn eq(&self, other: &Self) -> bool {
        self.unk_id == other.unk_id && self.vocab == other.vocab
    }
}

impl Clone for Unigram {
    // `Clone` can't be derive because it's not implemented for `Cache`.
    // To keep things simple when we clone, the new Unigram will start with a fresh cache.
    fn clone(&self) -> Self {
        let fresh_cache = self.cache.fresh();
        Self {
            vocab: self.vocab.clone(),
            cache: fresh_cache,
            token_to_ids: self.token_to_ids.clone(),
            trie: self.trie.clone(),
            min_score: self.min_score,
            unk_id: self.unk_id,
            bos_id: self.bos_id,
            eos_id: self.eos_id,
            fuse_unk: self.fuse_unk,
            is_optimized: self.is_optimized,
            byte_fallback: self.byte_fallback,
        }
    }
}

impl std::fmt::Debug for Unigram {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Unigram")
            .field("vocab", &self.vocab.len())
            .field("unk_id", &self.unk_id)
            .field("byte_fallback", &self.byte_fallback)
            .finish()
    }
}

static K_UNK_PENALTY: f64 = 10.0;

#[derive(Debug)]
pub enum UnigramError {
    EmptyVocabulary,
    UnkIdNotInVocabulary,
    MissingUnkId,
}

impl Default for Unigram {
    fn default() -> Self {
        let vocab = vec![("<unk>".to_string(), 0.0)];
        Self::from(vocab, Some(0), false).unwrap()
    }
}

impl Unigram {
    pub fn from(
        vocab: Vec<(String, f64)>,
        unk_id: Option<usize>,
        byte_fallback: bool,
    ) -> Result<Self> {
        let n = vocab.len();
        let mut token_to_ids: TokenMap = HashMap::new();
        let mut builder = TrieBuilder::default();

        if let Some(unk_id) = unk_id {
            if vocab.is_empty() {
                return Err(Box::new(UnigramError::EmptyVocabulary));
            }
            if unk_id >= vocab.len() {
                return Err(Box::new(UnigramError::UnkIdNotInVocabulary));
            }
        }
        let bos_id = n + 1;
        let eos_id = n + 2;

        let mut min_score = f64::INFINITY;
        for (id, (token, score)) in vocab.iter().enumerate() {
            token_to_ids.insert(token.to_string(), id as u32);
            builder.push(token.as_bytes());
            if score < &min_score {
                min_score = *score;
            }
        }
        let trie = builder.build();
        let fuse_unk = true;
        let is_optimized = true;

        Ok(Self {
            vocab,
            token_to_ids,
            trie,
            min_score,
            bos_id,
            eos_id,
            unk_id,
            fuse_unk,
            cache: Cache::default(),
            is_optimized,
            byte_fallback,
        })
    }

    #[cfg(test)]
    pub(super) fn set_fuse_unk(&mut self, fuse_unk: bool) {
        self.fuse_unk = fuse_unk;
        self.cache = self.cache.fresh();
    }

    #[cfg(test)]
    pub(super) fn set_optimized(&mut self, is_optimized: bool) {
        self.is_optimized = is_optimized;
    }
    pub fn byte_fallback(&self) -> bool {
        self.byte_fallback
    }
    pub(super) fn len(&self) -> usize {
        self.vocab.len()
    }

    pub(super) fn populate_nodes(&self, lattice: &mut Lattice) {
        let unk_score = self.min_score - K_UNK_PENALTY;

        let len = lattice.len();

        let mut begin_pos = 0;
        while begin_pos < len {
            let mblen = lattice.sentence[begin_pos..]
                .chars()
                .next()
                .unwrap()
                .len_utf8();

            let mut has_single_node = false;

            for bytes in self
                .trie
                .common_prefix_search(lattice.sentence.bytes().skip(begin_pos))
            {
                let n = bytes.len();
                let tok = String::from_utf8(bytes).unwrap();
                let id = *self.token_to_ids.get(&tok).unwrap();

                let item = &self.vocab[id as usize];
                assert_eq!(item.0, tok);
                let score: f64 = item.1;
                lattice.insert(begin_pos, n, score, id.try_into().unwrap());
                if !has_single_node && n == mblen {
                    has_single_node = true;
                }
            }

            if !has_single_node {
                if let Some(unk_id) = self.unk_id {
                    lattice.insert(begin_pos, mblen, unk_score, unk_id);
                }
            }
            begin_pos += mblen
        }
    }

    pub fn encode(&self, sentence: &str) -> Result<Vec<String>> {
        if sentence.is_empty() {
            return Ok(vec![]);
        }
        if let Some(result) = self.cache.get(sentence) {
            Ok(result.to_vec())
        } else {
            let result = if self.is_optimized {
                self.encode_optimized(sentence)?
            } else {
                self.encode_unoptimized(sentence)?
            };
            if sentence.len() < MAX_LENGTH {
                self.cache.set(sentence.to_owned(), result.clone());
            }
            Ok(result)
        }
    }

    fn encode_optimized(&self, sentence: &str) -> Result<Vec<String>> {
        // https://github.com/google/sentencepiece/blob/d48247191a6d50e469ed1a4a36e877befffd1851/src/unigram_model.cc#L600
        #[derive(Debug, Clone)]
        struct BestPathNode {
            id: usize,

            best_path_score: f64,

            starts_at: Option<usize>,
        }
        impl Default for BestPathNode {
            fn default() -> Self {
                Self {
                    id: 0,
                    best_path_score: 0.0,
                    starts_at: None,
                }
            }
        }
        let size = sentence.len();
        let unk_score = self.min_score - K_UNK_PENALTY;

        let mut best_path_ends_at = vec![BestPathNode::default(); size + 1];
        let mut starts_at = 0;
        while starts_at < size {
            let best_path_score_till_here = best_path_ends_at[starts_at].best_path_score;
            let mut has_single_node = false;
            let mblen = sentence[starts_at..].chars().next().unwrap().len_utf8();
            for tok_bytes in self
                .trie
                .common_prefix_search(sentence.bytes().skip(starts_at))
            {
                let key_pos = starts_at + tok_bytes.len();
                let token: String = String::from_utf8(tok_bytes).unwrap();
                let target_node = &mut best_path_ends_at[key_pos];
                let length = key_pos - starts_at;
                let id = self.token_to_ids.get(&token).unwrap();
                let score = self.vocab.get(*id as usize).unwrap().1;
                let candidate_best_path_score = score + best_path_score_till_here;
                if target_node.starts_at.is_none()
                    || candidate_best_path_score > target_node.best_path_score
                {
                    target_node.best_path_score = candidate_best_path_score;
                    target_node.starts_at = Some(starts_at);
                    target_node.id = *id as usize;
                }
                if !has_single_node && length == mblen {
                    has_single_node = true;
                }
            }
            if !has_single_node {
                let target_node = &mut best_path_ends_at[starts_at + mblen];
                let candidate_best_path_score = unk_score + best_path_score_till_here;
                if target_node.starts_at.is_none()
                    || candidate_best_path_score > target_node.best_path_score
                {
                    target_node.best_path_score = candidate_best_path_score;
                    target_node.starts_at = Some(starts_at);
                    target_node.id = self.unk_id.ok_or(UnigramError::MissingUnkId)?;
                }
            }
            starts_at += mblen
        }
        let mut ends_at = size;
        let mut results: Vec<String> = vec![];
        let mut token = vec![];
        while ends_at > 0 {
            let node = &best_path_ends_at[ends_at];
            let starts_at = node.starts_at.unwrap();
            if self.fuse_unk && Some(node.id) == self.unk_id {
                token.push(sentence[starts_at..ends_at].to_string());
            } else {
                if !token.is_empty() {
                    token.reverse();
                    results.push(token.concat());
                    token = vec![];
                }
                results.push(sentence[starts_at..ends_at].to_string());
            }
            ends_at = starts_at;
        }
        if !token.is_empty() {
            token.reverse();
            results.push(token.concat());
        }
        results.reverse();
        Ok(results)
    }

    fn encode_unoptimized(&self, sentence: &str) -> Result<Vec<String>> {
        let mut lattice = Lattice::from(sentence, self.bos_id, self.eos_id);
        self.populate_nodes(&mut lattice);
        if self.fuse_unk {
            let mut results = vec![];
            let mut token = String::new();
            for node in lattice.viterbi().iter() {
                let item = lattice.piece(&node.borrow());
                if node.borrow().id == self.unk_id.ok_or(UnigramError::MissingUnkId)? {
                    token.push_str(&item);
                } else {
                    if !token.is_empty() {
                        results.push(token);
                        token = String::new();
                    }
                    results.push(item);
                }
            }
            if !token.is_empty() {
                results.push(token);
            }
            Ok(results)
        } else {
            Ok(lattice.tokens())
        }
    }

    pub fn iter(&self) -> UnigramIterator<'_> {
        UnigramIterator { model: self, i: 0 }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn resize_cache(&mut self, capacity: usize) {
        self.cache.resize(capacity);
    }
}

pub struct UnigramIterator<'a> {
    model: &'a Unigram,
    i: usize,
}

impl<'a> Iterator for UnigramIterator<'a> {
    type Item = &'a (String, f64);

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.i;
        if i < self.model.len() {
            let r = Some(&self.model.vocab[i]);
            self.i += 1;
            r
        } else {
            None
        }
    }
}

impl Model for Unigram {
    fn get_vocab(&self) -> HashMap<String, u32> {
        self.token_to_ids.clone().into_iter().collect()
    }

    fn get_vocab_size(&self) -> usize {
        self.vocab.len()
    }

    fn tokenize(&self, sentence: &str) -> Result<Vec<Token>> {
        let str_tokens = self.encode(sentence)?;
        let mut offset = 0;
        let mut tokens = Vec::with_capacity(str_tokens.len());
        for string in str_tokens {
            let len = string.len();
            let offsets = (offset, offset + len);
            let id: u32 = match self.token_to_ids.get(&string) {
                Some(id) => *id,
                None => {
                    if self.byte_fallback {
                        let byte_tokens: Option<Vec<_>> = string
                            .bytes()
                            .map(|byte| -> Option<Token> {
                                let byte_string = format!("<0x{byte:02X}>");
                                let id = self.token_to_ids.get(&byte_string);
                                id.map(|id| Token::new(*id, byte_string, (offset, offset + len)))
                            })
                            .collect();
                        if let Some(byte_tokens) = byte_tokens {
                            for token in byte_tokens {
                                tokens.push(token);
                            }
                            offset += len;
                            continue;
                        }
                    }
                    self.unk_id.ok_or(UnigramError::MissingUnkId)? as u32
                }
            };
            offset += len;
            tokens.push(Token::new(id, string, offsets));
        }
        Ok(tokens)
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.token_to_ids.get(token).copied()
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        self.vocab.get(id as usize).map(|item| item.0.clone())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/unigram/model/tests.rs"]
mod tests;

impl std::fmt::Display for UnigramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyVocabulary => {
                f.write_str("The vocabulary is empty but at least <unk> is needed")
            }
            Self::UnkIdNotInVocabulary => {
                f.write_str("The `unk_id` is larger than vocabulary size")
            }
            Self::MissingUnkId => {
                f.write_str("Encountered an unknown token but `unk_id` is missing")
            }
        }
    }
}

impl std::error::Error for UnigramError {}

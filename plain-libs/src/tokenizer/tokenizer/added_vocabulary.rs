use super::{
    Model, NormalizedString, Normalizer, Offsets, PreTokenizedString, Token, normalizer::Range,
};
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::Regex;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeSeq};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddedToken {
    pub content: String,

    pub single_word: bool,

    pub lstrip: bool,

    pub rstrip: bool,

    pub normalized: bool,

    pub special: bool,
}

impl AddedToken {
    pub fn from<S: Into<String>>(content: S, special: bool) -> Self {
        Self {
            content: content.into(),
            normalized: !special,
            special,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn single_word(mut self, single_word: bool) -> Self {
        self.single_word = single_word;
        self
    }

    #[must_use]
    pub fn lstrip(mut self, lstrip: bool) -> Self {
        self.lstrip = lstrip;
        self
    }

    #[must_use]
    pub fn rstrip(mut self, rstrip: bool) -> Self {
        self.rstrip = rstrip;
        self
    }

    #[must_use]
    pub fn normalized(mut self, normalized: bool) -> Self {
        self.normalized = normalized;
        self
    }

    #[must_use]
    pub fn special(mut self, special: bool) -> Self {
        self.special = special;
        self
    }
}
impl Default for AddedToken {
    fn default() -> Self {
        Self {
            content: String::new(),
            single_word: false,
            lstrip: false,
            rstrip: false,
            normalized: true,
            special: false,
        }
    }
}
// AddedTokens can be updated if value changed
impl std::hash::Hash for AddedToken {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content.hash(state);
    }
}

type MatchingSet = (AhoCorasick, Vec<u32>);

static STARTS_WITH_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\w").unwrap());
static ENDS_WITH_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w$").unwrap());
static RIGHTMOST_SPACE_AT_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*").unwrap());
static LEFTMOST_SPACE_AT_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*$").unwrap());

fn ends_with_word(sentence: &str) -> bool {
    ENDS_WITH_WORD.is_match(sentence)
}

fn starts_with_word(sentence: &str) -> bool {
    STARTS_WITH_WORD.is_match(sentence)
}

fn space_leftmost_at_end(sentence: &str) -> usize {
    if let Some(match_) = LEFTMOST_SPACE_AT_END.find(sentence) {
        match_.start()
    } else {
        sentence.len()
    }
}
fn space_rightmost_at_start(sentence: &str) -> usize {
    if let Some(match_) = RIGHTMOST_SPACE_AT_START.find(sentence) {
        match_.end()
    } else {
        0
    }
}

#[derive(Clone, Debug)]
pub struct AddedVocabulary {
    added_tokens_map: HashMap<String, u32>,

    added_tokens_map_r: HashMap<u32, AddedToken>,

    added_tokens: Vec<AddedToken>,

    special_tokens: Vec<AddedToken>,

    special_tokens_set: HashSet<String>,

    split_trie: MatchingSet,

    split_normalized_trie: MatchingSet,

    encode_special_tokens: bool,
}

impl AddedVocabulary {
    pub fn new() -> Self {
        let trie = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build::<_, &&[u8]>([])
            .expect("The trie should build correctly");
        let normalized_trie = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build::<_, &&[u8]>([])
            .expect("The normalized trie should build correctly");
        Self {
            added_tokens_map: HashMap::new(),
            added_tokens_map_r: HashMap::new(),
            added_tokens: vec![],
            special_tokens: vec![],
            special_tokens_set: HashSet::new(),
            split_trie: (trie, vec![]),
            split_normalized_trie: (normalized_trie, vec![]),
            encode_special_tokens: false,
        }
    }

    // Suppress the "method is never used" warning
    pub fn len(&self) -> usize {
        self.added_tokens_map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.added_tokens_map.is_empty()
    }

    pub fn get_vocab(&self) -> &HashMap<String, u32> {
        &self.added_tokens_map
    }

    pub fn get_added_tokens_decoder(&self) -> &HashMap<u32, AddedToken> {
        &self.added_tokens_map_r
    }

    pub fn token_to_id(&self, token: &str, model: &impl Model) -> Option<u32> {
        self.added_tokens_map
            .get(token)
            .copied()
            .or_else(|| model.token_to_id(token))
    }

    #[deprecated(
        since = "0.19.0",
        note = "please use `added_vocabulary.simple_id_to_token(id).or_else(|| model.id_to_token(id)` instead"
    )]
    pub fn id_to_token(&self, id: u32, model: &impl Model) -> Option<String> {
        self.added_tokens_map_r
            .get(&id)
            .map(|t| t.content.clone())
            .or_else(|| model.id_to_token(id))
    }

    pub fn simple_id_to_token(&self, id: u32) -> Option<String> {
        self.added_tokens_map_r.get(&id).map(|t| t.content.clone())
    }

    //
    pub fn set_encode_special_tokens(&mut self, value: bool) {
        self.encode_special_tokens = value;
    }

    pub fn get_encode_special_tokens(&self) -> bool {
        self.encode_special_tokens
    }

    pub fn is_special_token(&self, token: &str) -> bool {
        self.special_tokens_set.contains(token)
    }

    pub fn add_special_tokens<N: Normalizer>(
        &mut self,
        tokens: &[AddedToken],
        model: &impl Model,
        normalizer: Option<&N>,
    ) -> usize {
        self.add_tokens(tokens, model, normalizer)
    }

    pub fn add_tokens<N: Normalizer>(
        &mut self,
        tokens: &[AddedToken],
        model: &impl Model,
        normalizer: Option<&N>,
    ) -> usize {
        // Handle special tokens (if any)
        for token in tokens {
            if token.special
                && !token.content.is_empty()
                && !self.special_tokens_set.contains(&token.content)
            {
                self.special_tokens.push(token.to_owned());
                self.special_tokens_set.insert(token.content.clone());
            }
        }

        let mut ignored = 0;

        let mut existing: HashSet<AddedToken> = self.added_tokens_map_r.values().cloned().collect();
        let mut next_id = self.added_tokens_map_r.keys().copied().max().map_or(
            model.get_vocab_size() as u32,
            |max| {
                if max >= model.get_vocab_size() as u32 || model.get_vocab_size() == 0 {
                    max + 1
                } else {
                    model.get_vocab_size() as u32
                }
            },
        );

        for token in tokens {
            if token.content.is_empty() || existing.contains(token) {
                ignored += 1;
                continue;
            }

            let new_id = if let Some(new_id) = self.token_to_id(&token.content, model) {
                new_id
            } else {
                let id = next_id;
                next_id += 1;
                id
            };

            *self
                .added_tokens_map
                .entry(token.content.clone())
                .or_default() = new_id;
            // Update the current revert operation
            *self.added_tokens_map_r.entry(new_id).or_default() = token.clone();
            // Make sure to remove previous entry (if the token gets a new id)

            // Finally add the token to the classic set if special
            if !self.special_tokens_set.contains(&token.content) {
                self.added_tokens.push(token.clone());
            }
            existing.insert(token.clone());
        }

        self.refresh_added_tokens(model, normalizer);

        // Return the number of added tokens
        tokens.len() - ignored
    }

    fn refresh_added_tokens<N: Normalizer>(&mut self, model: &impl Model, normalizer: Option<&N>) {
        type TupleTokenId<'a> = (&'a AddedToken, u32);
        let (normalized, non_normalized): (Vec<TupleTokenId>, Vec<TupleTokenId>) = self
            .special_tokens
            .iter()
            .chain(self.added_tokens.iter())
            .map(|token| {
                (
                    token,
                    self.token_to_id(&token.content, model)
                        .expect("Missing additional token"),
                )
            })
            .partition(|(token, _)| token.normalized);

        let (tokens, ids): (Vec<&AddedToken>, Vec<u32>) = non_normalized.into_iter().unzip();
        let trie = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(tokens.iter().map(|token| &token.content))
            .expect("Failed to build tried when refreshing tokens");
        self.split_trie = (trie, ids);

        let (ntokens, nids): (Vec<&AddedToken>, Vec<u32>) = normalized.into_iter().unzip();
        let patterns: Vec<_> = ntokens
            .iter()
            .map(|token| {
                let mut content = NormalizedString::from(token.content.as_ref());
                if let Some(n) = normalizer {
                    n.normalize(&mut content).unwrap();
                }
                content
            })
            .collect();
        let normalized_trie = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns.iter().map(|content| content.get()))
            .expect("Failed to build tried when refreshing tokens (normalized)");
        self.split_normalized_trie = (normalized_trie, nids);
    }

    fn find_matches(&self, sentence: &str, split_re: &MatchingSet) -> Vec<(Option<u32>, Offsets)> {
        if sentence.is_empty() {
            return vec![(None, (0, 0))];
        }

        let mut start_offset = 0;
        let mut splits = vec![];

        for mat in split_re.0.find_iter(sentence) {
            let mut start = mat.start();
            let mut stop = mat.end();
            let aho_id = mat.pattern();
            let id = split_re.1[aho_id];
            let added_token = &self.added_tokens_map_r.get(&id).unwrap();

            if self.encode_special_tokens && self.special_tokens_set.contains(&added_token.content)
            {
                continue;
            }

            if added_token.single_word {
                let start_space = start == 0 || !ends_with_word(&sentence[..start]);
                let stop_space = stop == sentence.len() || !starts_with_word(&sentence[stop..]);

                if !stop_space || !start_space {
                    // Discard not single word
                    continue;
                }
            }
            if added_token.lstrip {
                // This will be strictly inferior to start and in correct sentence offset
                let newstart = space_leftmost_at_end(&sentence[..start]);

                // The previous match could have already matched those spaces
                // Ignore them if it's already matched
                start = std::cmp::max(newstart, start_offset);
            }
            if added_token.rstrip {
                // This will starting a the stop+1 character, so we need
                // to add the previous stop value
                stop += space_rightmost_at_start(&sentence[stop..])
            }
            if start_offset < start {
                splits.push((None, (start_offset, start)));
            }
            splits.push((Some(id), (start, stop)));
            start_offset = stop;
        }

        let total_byte_len = sentence.len();
        if start_offset != total_byte_len {
            splits.push((None, (start_offset, total_byte_len)));
        }

        splits
    }

    fn split_with_indices(
        &self,
        sentence: NormalizedString,
        split_re: &MatchingSet,
    ) -> Vec<(NormalizedString, Option<Vec<Token>>)> {
        self.find_matches(sentence.get(), split_re)
            .into_iter()
            .map(|(id, byte_offsets)| {
                let slice = sentence
                    .slice(Range::Normalized(byte_offsets.0..byte_offsets.1))
                    .expect("AddedVocabulary bad split");
                if let Some(id) = id {
                    let value = slice.get().to_owned();
                    let len = value.len();
                    (slice, Some(vec![Token::new(id, value, (0, len))]))
                } else {
                    (slice, None)
                }
            })
            .collect()
    }

    pub fn extract_and_normalize<N: Normalizer>(
        &self,
        normalizer: Option<&N>,
        sequence: &str,
    ) -> PreTokenizedString {
        let mut pretokenized: PreTokenizedString = sequence.into();

        // 1. We extract all the non-normalized tokens from the non-normalized string
        pretokenized
            .split(|_, sequence| Ok(self.split_with_indices(sequence, &self.split_trie)))
            .expect("AddedVocabulary bad split");

        // <s> normalized = False
        // "I read a book   <s>Hey" -> "I read a book", "   <s>", "Hey"

        // </s> normalized = True -> "▁</s>"
        // "I read a book</s>Hey" -> "I read a book</s>Hey"

        // Day normalized = True -> "Day"
        // "I read a book monday" -> "I read a book monday"

        // [DAY] normalized = False -> "Day"
        // "I read a [DAY] monday" -> "I read a " "[DAY]", "book monday"
        //                                         320055
        // 2. Then extract the normalized tokens from the normalized pieces of the string
        pretokenized
            .split(|_, mut sequence| {
                normalizer.map(|n| n.normalize(&mut sequence));
                Ok(self.split_with_indices(sequence, &self.split_normalized_trie))
            })
            .expect("AddedVocabulary bad split");

        // ["I read a book", "   <s>", "Hey"] -> ["▁I read a book", "▁   <s>", "▁Hey"]
        // ["▁I read a book", "▁   <s>", "▁Hey"] -> [.., "▁   ", "<s>", "▁Hey"]

        // </s> normalized = True -> "▁</s>"
        // "I read a book</s>Hey" -> ["▁I read a book", "<","/","s",">", "Hey"]

        // "I read a " "[DAY]", "book monday" -> "i read a " "[day]", "book monday"

        pretokenized
    }
}

impl Default for AddedVocabulary {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AddedTokenWithId {
    pub id: u32,
    #[serde(flatten)]
    pub token: AddedToken,
}

impl Serialize for AddedVocabulary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut added_tokens = self
            .added_tokens_map_r
            .iter()
            .map(|(id, token)| AddedTokenWithId {
                id: *id,
                token: token.clone(),
            })
            .collect::<Vec<_>>();
        // We need to have these added tokens ordered by ascending ID
        added_tokens.sort_unstable_by_key(|o| o.id);

        let mut vocabulary = serializer.serialize_seq(Some(added_tokens.len()))?;
        for token in added_tokens {
            vocabulary.serialize_element(&token)?;
        }

        vocabulary.end()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/tokenizer/added_vocabulary/tests.rs"]
mod tests;

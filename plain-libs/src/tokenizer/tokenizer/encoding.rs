use crate::tokenizer::tokenizer::{Offsets, Token};
use crate::tokenizer::utils::padding::PaddingDirection;
use crate::tokenizer::utils::truncation::TruncationDirection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Encoding {
    ids: Vec<u32>,

    type_ids: Vec<u32>,

    tokens: Vec<String>,

    words: Vec<Option<u32>>,

    offsets: Vec<Offsets>,

    special_tokens_mask: Vec<u32>,

    attention_mask: Vec<u32>,

    overflowing: Vec<Encoding>,

    sequence_ranges: HashMap<usize, Range<usize>>,
}
impl Encoding {
    pub fn new(
        ids: Vec<u32>,
        type_ids: Vec<u32>,
        tokens: Vec<String>,
        words: Vec<Option<u32>>,
        offsets: Vec<Offsets>,
        special_tokens_mask: Vec<u32>,
        attention_mask: Vec<u32>,
        overflowing: Vec<Self>,
        sequence_ranges: HashMap<usize, Range<usize>>,
    ) -> Self {
        Self {
            ids,
            type_ids,
            tokens,
            words,
            offsets,
            special_tokens_mask,
            attention_mask,
            overflowing,
            sequence_ranges,
        }
    }

    pub fn with_capacity(len: usize) -> Self {
        Self {
            ids: Vec::with_capacity(len),
            type_ids: Vec::with_capacity(len),
            tokens: Vec::with_capacity(len),
            words: Vec::with_capacity(len),
            offsets: Vec::with_capacity(len),
            special_tokens_mask: Vec::with_capacity(len),
            attention_mask: Vec::with_capacity(len),
            overflowing: vec![],
            sequence_ranges: HashMap::new(),
        }
    }

    pub fn from_tokens(tokens: Vec<Token>, type_id: u32) -> Self {
        let length = tokens.len();
        let (ids, tokens, offsets) = tokens.into_iter().fold(
            (
                Vec::with_capacity(length),
                Vec::with_capacity(length),
                Vec::with_capacity(length),
            ),
            |(mut ids, mut tokens, mut offsets), t| {
                ids.push(t.id);
                tokens.push(t.value);
                offsets.push(t.offsets);
                (ids, tokens, offsets)
            },
        );

        Self {
            ids,
            tokens,
            offsets,
            words: vec![None; length],
            type_ids: vec![type_id; length],
            attention_mask: vec![1; length],
            special_tokens_mask: vec![0; length],
            overflowing: vec![],
            sequence_ranges: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn n_sequences(&self) -> usize {
        if self.sequence_ranges.is_empty() {
            1
        } else {
            self.sequence_ranges.len()
        }
    }

    pub fn set_sequence_id(&mut self, sequence_id: usize) {
        self.sequence_ranges.insert(sequence_id, 0..self.len());
    }

    pub fn get_tokens(&self) -> &[String] {
        &self.tokens[..]
    }

    pub fn get_word_ids(&self) -> &[Option<u32>] {
        &self.words
    }

    pub fn get_word_ids_mut(&mut self) -> &mut [Option<u32>] {
        &mut self.words
    }

    pub fn get_sequence_ids(&self) -> Vec<Option<usize>> {
        let mut sequences = vec![None; self.len()];
        for seq_id in 0..self.n_sequences() {
            let range = self.sequence_range(seq_id);
            let seq_len = range.len();
            sequences.splice(range, std::iter::repeat_n(Some(seq_id), seq_len));
        }
        sequences
    }

    pub fn get_ids(&self) -> &[u32] {
        &self.ids
    }

    pub fn get_type_ids(&self) -> &[u32] {
        &self.type_ids
    }

    pub fn set_type_ids(&mut self, type_ids: Vec<u32>) {
        self.type_ids = type_ids;
    }

    pub fn get_offsets(&self) -> &[Offsets] {
        &self.offsets
    }

    pub fn get_offsets_mut(&mut self) -> &mut [Offsets] {
        &mut self.offsets
    }

    pub fn get_special_tokens_mask(&self) -> &[u32] {
        &self.special_tokens_mask
    }

    pub fn get_attention_mask(&self) -> &[u32] {
        &self.attention_mask
    }

    pub fn get_overflowing(&self) -> &Vec<Encoding> {
        &self.overflowing
    }

    pub fn set_overflowing(&mut self, overflowing: Vec<Encoding>) {
        self.overflowing = overflowing;
    }

    pub fn get_overflowing_mut(&mut self) -> &mut Vec<Encoding> {
        &mut self.overflowing
    }

    pub fn take_overflowing(&mut self) -> Vec<Encoding> {
        std::mem::take(&mut self.overflowing)
    }

    pub(crate) fn process_tokens_with_offsets_mut<F>(&mut self, func: F)
    where
        F: FnMut((usize, (&String, &mut Offsets))),
    {
        self.tokens
            .iter()
            .zip(self.offsets.iter_mut())
            .enumerate()
            .for_each(func)
    }

    fn sequence_range(&self, sequence_id: usize) -> Range<usize> {
        self.sequence_ranges
            .get(&sequence_id)
            .cloned()
            .unwrap_or(0..self.len())
    }

    pub fn token_to_sequence(&self, token: usize) -> Option<usize> {
        if token > self.len() {
            None
        } else if self.sequence_ranges.is_empty() {
            Some(0)
        } else {
            self.sequence_ranges.iter().find_map(|(seq_id, range)| {
                if range.contains(&token) {
                    Some(*seq_id)
                } else {
                    None
                }
            })
        }
    }

    pub fn word_to_tokens(&self, word: u32, sequence_id: usize) -> Option<(usize, usize)> {
        let (mut start, mut end) = (None, None);
        let sequence_range = self.sequence_range(sequence_id);

        self.words
            .get(sequence_range.clone())?
            .iter()
            .enumerate()
            .take_while(|(_, w)| **w <= Some(word))
            .filter(|(_, w)| **w == Some(word))
            .for_each(|(i, _)| {
                if start.is_none() || Some(i) < start {
                    start = Some(i);
                }
                if end.is_none() || Some(i) >= end {
                    end = Some(i + 1);
                }
            });

        if let (Some(start), Some(end)) = (start, end) {
            Some((sequence_range.start + start, sequence_range.start + end))
        } else {
            None
        }
    }

    pub fn word_to_chars(&self, word: u32, sequence_id: usize) -> Option<Offsets> {
        self.word_to_tokens(word, sequence_id)
            .and_then(|(start, end)| {
                if end == 0 {
                    None
                } else {
                    Some((self.offsets[start].0, self.offsets[end - 1].1))
                }
            })
    }

    pub fn token_to_chars(&self, token: usize) -> Option<(usize, Offsets)> {
        Some((
            self.token_to_sequence(token)?,
            self.offsets.get(token).copied()?,
        ))
    }

    pub fn token_to_word(&self, token: usize) -> Option<(usize, u32)> {
        Some((
            self.token_to_sequence(token)?,
            self.words.get(token).copied().flatten()?,
        ))
    }

    pub fn char_to_token(&self, pos: usize, sequence_id: usize) -> Option<usize> {
        let sequence_range = self.sequence_range(sequence_id);

        self.offsets
            .get(sequence_range.clone())?
            .iter()
            .position(|(start, end)| pos >= *start && pos < *end)
            .map(|pos| sequence_range.start + pos)
    }

    pub fn char_to_word(&self, pos: usize, sequence_id: usize) -> Option<u32> {
        Some(
            self.char_to_token(pos, sequence_id)
                .and_then(|token| self.token_to_word(token))?
                .1,
        )
    }

    pub fn truncate(&mut self, max_len: usize, stride: usize, direction: TruncationDirection) {
        let encoding_len = self.ids.len();
        if max_len >= encoding_len {
            return;
        }

        if max_len == 0 {
            let o = std::mem::replace(self, Encoding::with_capacity(0));
            self.overflowing.push(o);
            return;
        }

        assert!(
            stride < max_len,
            "`stride` must be strictly less than `max_len={}` (note that `max_len` may be shorter than the max length of the original model, as it subtracts the number of special characters",
            max_len
        );

        // When truncating, we lose the `sequence_ranges` information.
        self.sequence_ranges.clear();

        let offset = max_len - stride;
        let mut end = false;
        let parts_ranges: Vec<(usize, usize)> = match direction {
            TruncationDirection::Right => (0..encoding_len)
                .step_by(offset)
                .filter_map(|start| {
                    if !end {
                        let stop = std::cmp::min(start + max_len, encoding_len);
                        end = stop == encoding_len;
                        Some((start, stop))
                    } else {
                        None
                    }
                })
                .collect(),
            TruncationDirection::Left => (0..encoding_len)
                .rev()
                .step_by(offset)
                .filter_map(|stop| {
                    let stop = stop + 1;
                    let start = stop.saturating_sub(max_len);
                    if start < stop && !end {
                        end = start == 0;
                        Some((start, stop))
                    } else {
                        None
                    }
                })
                .collect(),
        };

        let mut i = 0;
        let (start, stop) = parts_ranges[i];
        let mut new_encoding = Encoding {
            ids: self.ids[start..stop].to_vec(),
            type_ids: self.type_ids[start..stop].to_vec(),
            tokens: self.tokens[start..stop].to_vec(),
            words: self.words[start..stop].to_vec(),
            offsets: self.offsets[start..stop].to_vec(),
            special_tokens_mask: self.special_tokens_mask[start..stop].to_vec(),
            attention_mask: self.attention_mask[start..stop].to_vec(),
            overflowing: vec![],
            sequence_ranges: HashMap::new(),
        };

        loop {
            if i == parts_ranges.len() - 1 {
                break;
            }
            i += 1;
            let (start, stop) = parts_ranges[i];
            new_encoding.overflowing.push(Encoding {
                ids: self.ids[start..stop].to_vec(),
                type_ids: self.type_ids[start..stop].to_vec(),
                tokens: self.tokens[start..stop].to_vec(),
                words: self.words[start..stop].to_vec(),
                offsets: self.offsets[start..stop].to_vec(),
                special_tokens_mask: self.special_tokens_mask[start..stop].to_vec(),
                attention_mask: self.attention_mask[start..stop].to_vec(),
                overflowing: vec![],
                sequence_ranges: HashMap::new(),
            });
        }
        *self = new_encoding;
    }

    pub fn merge<I: IntoIterator<Item = Encoding>>(encodings: I, growing_offsets: bool) -> Self {
        let mut encoding = Encoding::default();

        // TODO this is suboptimal as we're doing this iteratively instead of preallocating
        // all the encodings sizes all at once and only copying into this preallocated vector
        // https://github.com/huggingface/tokenizers/pull/1049

        // In order to fix, we just need to preallocate all vectors, then copy everything
        // into it (and deal with overlowings correctly)
        for sub in encodings {
            encoding.merge_with(sub, growing_offsets);
        }

        encoding
    }

    pub fn merge_with(&mut self, pair: Encoding, growing_offsets: bool) {
        // Handle merging the overflowing parts too: Combine them all
        // In most of the cases, we expect `pair.overflowing.len() == 0`
        let mut overflowings = vec![];

        // 1. All our overflowings with all the others
        for self_o in &self.overflowing {
            // 1. The pair itself
            let mut n_encoding = self_o.clone();
            n_encoding.merge_with(pair.clone(), growing_offsets);
            overflowings.push(n_encoding);

            // 2. Its overflowings (this should rarely happen...)
            for other_o in &pair.overflowing {
                let mut n_encoding = self_o.clone();
                n_encoding.merge_with(other_o.clone(), growing_offsets);
                overflowings.push(n_encoding);
            }
        }
        // 2. Ourself with all the other overflowings (this should rarely happen too...)
        for other_o in &pair.overflowing {
            let mut n_encoding = self.clone();
            n_encoding.merge_with(other_o.clone(), growing_offsets);
            overflowings.push(n_encoding);
        }

        // Finish by merging ourself with the other encoding
        let original_self_len = self.len(); // Must be before any modification to self.ids

        self.sequence_ranges
            .extend(pair.sequence_ranges.into_iter().map(|(seq_id, range)| {
                (
                    seq_id,
                    original_self_len + range.start..original_self_len + range.end,
                )
            }));
        self.ids.extend(pair.ids);
        self.type_ids.extend(pair.type_ids);
        self.tokens.extend(pair.tokens);
        self.words.extend(pair.words);

        let starting_offset = if growing_offsets {
            self.offsets.last().map_or(0, |o| o.1)
        } else {
            0
        };
        self.offsets.extend(
            pair.offsets
                .into_iter()
                .map(|(start, end)| (start + starting_offset, end + starting_offset))
                .collect::<Vec<_>>(),
        );
        self.special_tokens_mask.extend(pair.special_tokens_mask);
        self.attention_mask.extend(pair.attention_mask);
        self.overflowing = overflowings;
    }

    pub fn pad(
        &mut self,
        target_length: usize,
        pad_id: u32,
        pad_type_id: u32,
        pad_token: &str,
        direction: PaddingDirection,
    ) {
        // Dispatch call to all the overflowings first
        self.overflowing.iter_mut().for_each(|encoding| {
            encoding.pad(target_length, pad_id, pad_type_id, pad_token, direction)
        });

        // Then check if we should pad ourself
        if self.ids.len() >= target_length {
            // We just do nothing if the wanted padding length is smaller than us
            return;
        }
        let pad_length = target_length - self.ids.len();

        match direction {
            PaddingDirection::Left => {
                self.ids = (0..pad_length)
                    .map(|_| pad_id)
                    .chain(self.ids.drain(..))
                    .collect();
                self.type_ids = (0..pad_length)
                    .map(|_| pad_type_id)
                    .chain(self.type_ids.drain(..))
                    .collect();
                self.tokens = (0..pad_length)
                    .map(|_| pad_token.to_owned())
                    .chain(self.tokens.drain(..))
                    .collect();
                self.words = (0..pad_length)
                    .map(|_| None)
                    .chain(self.words.drain(..))
                    .collect();
                self.attention_mask = (0..pad_length)
                    .map(|_| 0)
                    .chain(self.attention_mask.drain(..))
                    .collect();
                self.special_tokens_mask = (0..pad_length)
                    .map(|_| 1)
                    .chain(self.special_tokens_mask.drain(..))
                    .collect();
                self.offsets = (0..pad_length)
                    .map(|_| (0, 0))
                    .chain(self.offsets.drain(..))
                    .collect();
                self.sequence_ranges
                    .iter_mut()
                    .for_each(|(_seq_id, range)| {
                        *range = (range.start + pad_length)..(range.end + pad_length)
                    });
            }
            PaddingDirection::Right => {
                self.ids.extend((0..pad_length).map(|_| pad_id));
                self.type_ids.extend((0..pad_length).map(|_| pad_type_id));
                self.tokens
                    .extend((0..pad_length).map(|_| pad_token.to_owned()));
                self.words.extend((0..pad_length).map(|_| None));
                self.attention_mask.extend((0..pad_length).map(|_| 0));
                self.special_tokens_mask.extend((0..pad_length).map(|_| 1));
                self.offsets.extend((0..pad_length).map(|_| (0, 0)));
            }
        }
    }
}

impl std::iter::FromIterator<Encoding> for Encoding {
    fn from_iter<I: IntoIterator<Item = Encoding>>(iter: I) -> Self {
        Self::merge(iter, false)
    }
}

impl std::iter::FromIterator<(u32, String, (usize, usize), Option<u32>, u32)> for Encoding {
    fn from_iter<I: IntoIterator<Item = (u32, String, (usize, usize), Option<u32>, u32)>>(
        iter: I,
    ) -> Self {
        let items = iter.into_iter();
        let (lower, upper) = items.size_hint();
        let length = upper.unwrap_or(lower);
        let mut encoding = Self::with_capacity(length);

        for (id, token, offsets, word, type_id) in items {
            encoding.ids.push(id);
            encoding.tokens.push(token);
            encoding.offsets.push(offsets);
            encoding.type_ids.push(type_id);
            encoding.words.push(word);
            encoding.special_tokens_mask.push(0);
            encoding.attention_mask.push(1);
        }

        encoding
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/tokenizer/encoding/tests.rs"]
mod tests;

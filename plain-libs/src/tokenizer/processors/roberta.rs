use crate::tokenizer::processors::byte_level::process_offsets;
use crate::tokenizer::tokenizer::{Encoding, PostProcessor, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::iter::FromIterator;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub struct RobertaProcessing {
    pub sep: (String, u32),
    pub cls: (String, u32),
    pub trim_offsets: bool,
    pub add_prefix_space: bool,
}

impl Default for RobertaProcessing {
    fn default() -> Self {
        Self {
            sep: ("</s>".into(), 2),
            cls: ("<s>".into(), 0),
            trim_offsets: true,
            add_prefix_space: true,
        }
    }
}

impl RobertaProcessing {
    pub fn new(sep: (String, u32), cls: (String, u32)) -> Self {
        Self {
            sep,
            cls,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn trim_offsets(mut self, v: bool) -> Self {
        self.trim_offsets = v;
        self
    }

    #[must_use]
    pub fn add_prefix_space(mut self, v: bool) -> Self {
        self.add_prefix_space = v;
        self
    }

    pub fn get_sep_copy(&self) -> (String, u32) {
        (self.sep.0.clone(), self.sep.1)
    }

    pub fn get_cls_copy(&self) -> (String, u32) {
        (self.cls.0.clone(), self.cls.1)
    }
}

impl PostProcessor for RobertaProcessing {
    fn added_tokens(&self, is_pair: bool) -> usize {
        if is_pair { 4 } else { 2 }
    }

    fn process_encodings(
        &self,
        mut encodings: Vec<Encoding>,
        add_special_tokens: bool,
    ) -> Result<Vec<Encoding>> {
        if self.trim_offsets {
            for encoding in encodings.iter_mut() {
                process_offsets(encoding, self.add_prefix_space);
                encoding
                    .get_overflowing_mut()
                    .iter_mut()
                    .for_each(|encoding| process_offsets(encoding, self.add_prefix_space));
            }
        }

        // Roberta is weird, and every encoding is type_id=0.
        encodings
            .iter_mut()
            .for_each(|encoding| encoding.set_type_ids(vec![0; encoding.len()]));

        if !add_special_tokens {
            return Ok(encodings);
        }

        let encodings: Vec<Encoding> = encodings
            .iter_mut()
            .enumerate()
            .map(|(i, encoding)| {
                if i == 0 {
                    let ids = [&[self.cls.1], encoding.get_ids(), &[self.sep.1]].concat();
                    let type_ids = [&[0], encoding.get_type_ids(), &[0]].concat();
                    let tokens = [
                        std::slice::from_ref(&self.cls.0),
                        encoding.get_tokens(),
                        std::slice::from_ref(&self.sep.0),
                    ]
                    .concat();
                    let words = [&[None], encoding.get_word_ids(), &[None]].concat();
                    let offsets = [&[(0, 0)], encoding.get_offsets(), &[(0, 0)]].concat();
                    let special_tokens =
                        [&[1u32], &vec![0; encoding.get_ids().len()][..], &[1]].concat();
                    let attention_mask = vec![1; ids.len()];

                    // For compatibility with `TemplateProcessing`, the sequence_ranges shouldn't contain
                    // the special tokens.
                    let sequence_ranges = HashMap::from_iter(vec![(0, 1..ids.len() - 1)]);
                    Encoding::new(
                        ids,
                        type_ids,
                        tokens,
                        words,
                        offsets,
                        special_tokens,
                        attention_mask,
                        encoding
                            .take_overflowing()
                            .into_iter()
                            .map(|encoding| {
                                let ids =
                                    [&[self.cls.1], encoding.get_ids(), &[self.sep.1]].concat();
                                let type_ids = vec![0; encoding.get_ids().len() + 2];
                                let tokens = [
                                    std::slice::from_ref(&self.cls.0),
                                    encoding.get_tokens(),
                                    std::slice::from_ref(&self.sep.0),
                                ]
                                .concat();
                                let words = [&[None], encoding.get_word_ids(), &[None]].concat();
                                let offsets =
                                    [&[(0, 0)], encoding.get_offsets(), &[(0, 0)]].concat();
                                let special_tokens =
                                    [&[1u32], &vec![0; encoding.get_ids().len()][..], &[1]]
                                        .concat();
                                let attention_mask = vec![1; ids.len()];

                                // For compatibility with `TemplateProcessing`, the sequence_ranges shouldn't
                                // contain the special tokens.
                                let sequence_ranges =
                                    HashMap::from_iter(vec![(0, 1..ids.len() - 1)]);
                                Encoding::new(
                                    ids,
                                    type_ids,
                                    tokens,
                                    words,
                                    offsets,
                                    special_tokens,
                                    attention_mask,
                                    vec![],
                                    sequence_ranges,
                                )
                            })
                            .collect(),
                        sequence_ranges,
                    )
                } else {
                    let pair_ids = [&[self.sep.1], encoding.get_ids(), &[self.sep.1]].concat();
                    let pair_type_ids = vec![0; encoding.get_ids().len() + 2];
                    let pair_tokens = [
                        std::slice::from_ref(&self.sep.0),
                        encoding.get_tokens(),
                        std::slice::from_ref(&self.sep.0),
                    ]
                    .concat();
                    let pair_words = [&[None], encoding.get_word_ids(), &[None]].concat();
                    let pair_offsets = [&[(0, 0)], encoding.get_offsets(), &[(0, 0)]].concat();
                    let pair_special_tokens =
                        [&[1], &vec![0u32; encoding.get_type_ids().len()][..], &[1]].concat();
                    let pair_attention_mask = vec![1; pair_ids.len()];

                    // For compatibility with `TemplateProcessing`, the sequence_ranges shouldn't contain
                    // the special tokens.
                    let pair_sequence_ranges = HashMap::from_iter(vec![(1, 1..pair_ids.len() - 1)]);
                    Encoding::new(
                        pair_ids,
                        pair_type_ids,
                        pair_tokens,
                        pair_words,
                        pair_offsets,
                        pair_special_tokens,
                        pair_attention_mask,
                        encoding
                            .take_overflowing()
                            .into_iter()
                            .map(|encoding| {
                                let pair_ids =
                                    [&[self.sep.1], encoding.get_ids(), &[self.sep.1]].concat();
                                let pair_type_ids = vec![0; encoding.get_ids().len() + 2];
                                let pair_tokens = [
                                    std::slice::from_ref(&self.sep.0),
                                    encoding.get_tokens(),
                                    std::slice::from_ref(&self.sep.0),
                                ]
                                .concat();
                                let pair_words =
                                    [&[None], encoding.get_word_ids(), &[None]].concat();
                                let pair_offsets =
                                    [&[(0, 0)], encoding.get_offsets(), &[(0, 0)]].concat();
                                let pair_special_tokens =
                                    [&[1], &vec![0u32; encoding.get_type_ids().len()][..], &[1]]
                                        .concat();
                                let pair_attention_mask = vec![1; pair_ids.len()];

                                // For compatibility with `TemplateProcessing`, the sequence_ranges
                                // shouldn't contain the special tokens.
                                let pair_sequence_ranges =
                                    HashMap::from_iter(vec![(1, 1..pair_ids.len() - 1)]);
                                Encoding::new(
                                    pair_ids,
                                    pair_type_ids,
                                    pair_tokens,
                                    pair_words,
                                    pair_offsets,
                                    pair_special_tokens,
                                    pair_attention_mask,
                                    vec![],
                                    pair_sequence_ranges,
                                )
                            })
                            .collect(),
                        pair_sequence_ranges,
                    )
                }
            })
            .collect();

        Ok(encodings)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/processors/roberta/tests.rs"]
mod tests;

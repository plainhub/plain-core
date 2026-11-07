use crate::tokenizer::{
    Encoding, NormalizedString, OffsetReferential, Offsets, Result, Token, normalizer::Range,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetType {
    Byte,
    Char,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    normalized: NormalizedString,

    tokens: Option<Vec<Token>>,
}

impl From<NormalizedString> for Split {
    fn from(n: NormalizedString) -> Self {
        Self {
            normalized: n,
            tokens: None,
        }
    }
}

impl From<(NormalizedString, Option<Vec<Token>>)> for Split {
    fn from(f: (NormalizedString, Option<Vec<Token>>)) -> Self {
        Self {
            normalized: f.0,
            tokens: f.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTokenizedString {
    original: String,
    splits: Vec<Split>,
}

impl PreTokenizedString {
    pub fn split<F, U, R>(&mut self, mut split_fn: F) -> Result<()>
    where
        F: FnMut(usize, NormalizedString) -> Result<U>,
        U: IntoIterator<Item = R>,
        R: Into<Split>,
    {
        // new_splits is at least as big as self.splits
        let mut new_splits = Vec::with_capacity(self.splits.len());
        for (i, original_split) in self.splits.drain(..).enumerate() {
            if original_split.tokens.is_some() {
                new_splits.push(original_split);
                continue;
            }

            new_splits.extend(
                split_fn(i, original_split.normalized)?
                    .into_iter()
                    .filter_map(|split| {
                        let split: Split = split.into();
                        if split.normalized.is_empty() {
                            None
                        } else {
                            Some(split)
                        }
                    }),
            );
        }
        self.splits = new_splits;

        Ok(())
    }

    pub fn normalize<F>(&mut self, normalize: F) -> Result<()>
    where
        F: Fn(&mut NormalizedString) -> Result<()>,
    {
        for split in self.splits.iter_mut().filter(|s| s.tokens.is_none()) {
            normalize(&mut split.normalized)?;
        }
        Ok(())
    }

    pub fn tokenize<F>(&mut self, tokenize: F) -> Result<()>
    where
        F: Fn(&NormalizedString) -> Result<Vec<Token>>,
    {
        for split in self.splits.iter_mut().filter(|s| s.tokens.is_none()) {
            split.tokens = Some(tokenize(&split.normalized)?);
        }

        Ok(())
    }

    pub fn into_encoding(
        self,
        word_idx: Option<u32>,
        type_id: u32,
        offset_type: OffsetType,
    ) -> Result<Encoding> {
        if self.splits.is_empty() {
            Ok(Encoding::default())
        } else if !self.splits.iter().all(|split| split.tokens.is_some()) {
            Err("Split has not been tokenized, call `PreTokenizedString::tokenize` first".into())
        } else {
            let offset_converter = match offset_type {
                OffsetType::Char => Some(BytesToCharOffsetConverter::new(&self.original)),
                OffsetType::Byte => None,
                OffsetType::None => {
                    let tokens = self
                        .splits
                        .into_iter()
                        .flat_map(|split| {
                            split.tokens.unwrap().into_iter().map(|token| {
                                // Replace this with the actual fields you need for the Encoding type
                                (token.id, String::with_capacity(0), (0, 0), None, 0)
                            })
                        })
                        .collect();
                    return Ok(tokens);
                }
            };

            Ok(self
                .splits
                .into_iter()
                .enumerate()
                .flat_map(|(idx, split)| {
                    let normalized = split.normalized;
                    let offsets = normalized.offsets_original();
                    let offset_converter = &offset_converter;

                    split.tokens.unwrap().into_iter().map(move |token| {
                        let mut offsets = normalized
                            .convert_offsets(Range::Normalized(token.offsets.0..token.offsets.1))
                            .map_or(token.offsets, |range| {
                                (offsets.0 + range.start, offsets.0 + range.end)
                            });

                        // Convert to char offsets if relevant
                        if let Some(converter) = offset_converter {
                            offsets = converter.convert(offsets).unwrap_or(offsets);
                        }

                        (
                            token.id,
                            token.value,
                            offsets,
                            if word_idx.is_some() {
                                word_idx
                            } else {
                                Some(idx as u32)
                            },
                            type_id,
                        )
                    })
                })
                .collect())
        }
    }

    pub fn get_splits(
        &self,
        offset_ref: OffsetReferential,
        offset_type: OffsetType,
    ) -> Vec<(&str, Offsets, &Option<Vec<Token>>)> {
        let offset_converter = match offset_type {
            OffsetType::Char => Some(BytesToCharOffsetConverter::new(&self.original)),
            OffsetType::Byte => None,
            OffsetType::None => None,
        };

        let mut offset = 0;
        self.splits
            .iter()
            .map(|split| {
                let mut offsets = match offset_ref {
                    OffsetReferential::Original => split.normalized.offsets_original(),
                    OffsetReferential::Normalized => {
                        let len = split.normalized.len();
                        offset += len;
                        (offset - len, offset)
                    }
                };

                // Convert to char offsets if relevant
                if let Some(ref converter) = offset_converter {
                    offsets = converter.convert(offsets).unwrap_or(offsets);
                }

                (split.normalized.get(), offsets, &split.tokens)
            })
            .collect()
    }
}

impl From<NormalizedString> for PreTokenizedString {
    fn from(s: NormalizedString) -> Self {
        Self {
            original: s.get_original().to_owned(),
            splits: vec![Split {
                normalized: s,
                tokens: None,
            }],
        }
    }
}

impl From<&str> for PreTokenizedString {
    fn from(s: &str) -> Self {
        let normalized: NormalizedString = s.into();
        normalized.into()
    }
}

impl From<String> for PreTokenizedString {
    fn from(s: String) -> Self {
        let normalized: NormalizedString = s.into();
        normalized.into()
    }
}

struct BytesToCharOffsetConverter {
    map: HashMap<usize, usize>,
}

impl BytesToCharOffsetConverter {
    pub fn new(sequence: &str) -> Self {
        Self {
            map: sequence
                .char_indices()
                .enumerate()
                .flat_map(|(i, (b, c))| {
                    let mut n = 0;
                    std::iter::repeat_with(move || {
                        let o = (b + n, i);
                        n += 1;
                        o
                    })
                    .take(c.len_utf8())
                })
                .collect(),
        }
    }

    pub fn convert(&self, offsets: Offsets) -> Option<Offsets> {
        match (self.map.get(&offsets.0), self.map.get(&offsets.1)) {
            (Some(start), Some(end)) => Some((*start, *end)),
            // If we reached the end, `end` is not in the map
            (Some(start), None) => {
                // But the one just before should be
                let last = self.map.get(&(offsets.1 - 1)).copied().unwrap_or(start + 1);
                Some((*start, last + 1))
            }
            _ => None,
        }
    }
}

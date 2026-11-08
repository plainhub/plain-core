use crate::tokenizer::tokenizer::{NormalizedString, Normalizer, Result};
use crate::utils::base64::{base64_decode_standard_optional_padding, base64_encode};
use serde::{Deserialize, Serialize, Serializer};
use std::cmp::Ordering;
use crate::tokenizer::graphemes::graphemes;

// SentencePiece trie format adapted from spm_precompiled 0.1.4 (../spm_precompiled/SOURCE.json).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", try_from = "PrecompiledDeserializer")]
pub struct Precompiled {
    #[serde(serialize_with = "serialize_map")]
    precompiled_charsmap: Vec<u8>,
    #[serde(skip)]
    normalized: String,
    #[serde(skip)]
    trie: Vec<u32>,
}

#[derive(Deserialize)]
struct PrecompiledDeserializer {
    precompiled_charsmap: String,
}

fn serialize_map<S: Serializer>(
    bytes: &[u8],
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&base64_encode(bytes))
}

impl TryFrom<PrecompiledDeserializer> for Precompiled {
    type Error = &'static str;

    fn try_from(value: PrecompiledDeserializer) -> std::result::Result<Self, Self::Error> {
        let bytes = base64_decode_standard_optional_padding(&value.precompiled_charsmap)
            .map_err(|_| "Invalid Base64 precompiled_charsmap")?;
        let (header, contents) = bytes
            .split_first_chunk::<4>()
            .ok_or("Missing charsmap length")?;
        let trie_size = u32::from_le_bytes(*header) as usize;
        if trie_size == 0 || trie_size % 4 != 0 {
            return Err("Invalid charsmap trie length");
        }
        let (trie, normalized) = contents
            .split_at_checked(trie_size)
            .ok_or("Truncated charsmap trie")?;
        let trie: Vec<u32> = trie
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&bytes| u32::from_le_bytes(bytes))
            .collect();
        if offset(trie[0]) >= trie.len() {
            return Err("Invalid charsmap root offset");
        }
        let normalized = std::str::from_utf8(normalized)
            .map_err(|_| "Invalid charsmap replacement text")?
            .to_owned();
        Ok(Self {
            precompiled_charsmap: bytes,
            normalized,
            trie,
        })
    }
}

fn offset(unit: u32) -> usize {
    ((unit >> 10) << ((unit & (1 << 9)) >> 6)) as usize
}

impl Precompiled {
    fn transform(&self, chunk: &str) -> std::result::Result<Option<&str>, &'static str> {
        let mut node = offset(self.trie[0]);
        for byte in chunk.bytes().take_while(|&byte| byte != 0) {
            node ^= usize::from(byte);
            let unit = *self.trie.get(node).ok_or("Invalid charsmap trie offset")?;
            if unit & ((1 << 31) | 0xff) != u32::from(byte) {
                return Ok(None);
            }
            node ^= offset(unit);
            if unit & (1 << 8) != 0 {
                let leaf = *self.trie.get(node).ok_or("Invalid charsmap leaf offset")?;
                let index = (leaf & ((1 << 31) - 1)) as usize;
                let text = self
                    .normalized
                    .get(index..)
                    .ok_or("Invalid charsmap text offset")?;
                return Ok(Some(text.split_once('\0').map_or(text, |(part, _)| part)));
            }
        }
        Ok(None)
    }
}

fn replace(transformations: &mut Vec<(char, isize)>, old_part: &str, new_part: &str) {
    let old_count = old_part.chars().count() as isize;
    let new_count = new_part.chars().count() as isize;
    let diff = new_count - old_count;

    // If we are just replacing characters, all changes should be == 0
    transformations.extend(new_part.chars().map(|c| (c, 0)));

    match diff.cmp(&0) {
        // If we are adding some characters, the last DIFF characters should be == 1
        Ordering::Greater => {
            transformations
                .iter_mut()
                .rev()
                .take(diff as usize)
                .for_each(|(_, cs)| *cs = 1);
        }
        // If we are removing some characters, the last one should include the diff
        Ordering::Less => {
            if let Some((_, cs)) = transformations.last_mut() {
                *cs += diff;
            }
        }
        _ => {}
    }
}

impl Normalizer for Precompiled {
    fn normalize(&self, normalized: &mut NormalizedString) -> Result<()> {
        let mut transformations = Vec::with_capacity(normalized.get().len());
        // Future reader. From @Narsil.
        // Yes, this is weird,
        // Yes, this seems broken
        // No, I don't know why Google did this.
        // If you question this code, check this normalizer against
        // XNLI database (all languages) with Unigram model against
        // Mbart, XLMRoberta *AND* Marian. If you don't get 100% or
        // break a single test.
        // You don't pass.
        let mut modified = false;
        for grapheme in graphemes(normalized.get()) {
            if grapheme.len() < 6 {
                if let Some(norm) = self.transform(grapheme)? {
                    modified = true;
                    replace(&mut transformations, grapheme, norm);
                    continue;
                }
            }
            for (char_index, c) in grapheme.char_indices() {
                let part = &grapheme[char_index..char_index + c.len_utf8()];
                if let Some(norm) = self.transform(part)? {
                    modified = true;
                    replace(&mut transformations, part, norm);
                } else {
                    transformations.push((c, 0));
                }
            }
        }
        if modified {
            normalized.transform(transformations, 0);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/precompiled/tests.rs"]
mod tests;

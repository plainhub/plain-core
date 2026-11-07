use derive_builder::Builder;

use crate::tokenizer::{Encoding, PostProcessor, Result};
use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::result::Result as StdResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum Sequence {
    A,

    B,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum Piece {
    Sequence { id: Sequence, type_id: u32 },
    SpecialToken { id: String, type_id: u32 },
}

impl Piece {
    fn extract_id(s: &str) -> Option<Self> {
        if s.starts_with('$') {
            let rest = &s['$'.len_utf8()..];

            // If the id is just `$`, we use 0 as type_id, and Sequence A
            match rest {
                "" => Some(Self::Sequence {
                    id: Sequence::A,
                    type_id: 0,
                }),
                "A" | "a" => Some(Self::Sequence {
                    id: Sequence::A,
                    type_id: 0,
                }),
                "B" | "b" => Some(Self::Sequence {
                    id: Sequence::B,
                    type_id: 0,
                }),
                n => {
                    if let Ok(type_id) = n.parse::<u32>() {
                        Some(Self::Sequence {
                            id: Sequence::A,
                            type_id,
                        })
                    } else {
                        None
                    }
                }
            }
        } else {
            Some(Self::SpecialToken {
                id: s.to_owned(),
                type_id: 0,
            })
        }
    }

    fn with_type_id(self, type_id: u32) -> Self {
        match self {
            Self::Sequence { id, .. } => Self::Sequence { id, type_id },
            Self::SpecialToken { id, .. } => Self::SpecialToken { id, type_id },
        }
    }
}

impl TryFrom<String> for Piece {
    type Error = String;

    fn try_from(s: String) -> StdResult<Self, Self::Error> {
        let parts = s.split(':').collect::<Vec<_>>();

        let err = || format!("Cannot build Piece from string \"{s}\"");
        match parts.as_slice() {
            [id, type_id] => {
                let type_id: u32 = type_id.parse().map_err(|_| err())?;
                let piece = Self::extract_id(id).ok_or_else(err)?;
                Ok(piece.with_type_id(type_id))
            }
            [id] => Self::extract_id(id).ok_or_else(err),
            _ => Err(err()),
        }
    }
}

impl TryFrom<&str> for Piece {
    type Error = String;

    fn try_from(s: &str) -> StdResult<Self, Self::Error> {
        Piece::try_from(s.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct SpecialToken {
    id: String,

    ids: Vec<u32>,

    tokens: Vec<String>,
}

impl From<(String, u32)> for SpecialToken {
    fn from(v: (String, u32)) -> Self {
        Self {
            id: v.0.clone(),
            ids: vec![v.1],
            tokens: vec![v.0],
        }
    }
}
impl From<(&str, u32)> for SpecialToken {
    fn from(v: (&str, u32)) -> Self {
        Self::from((v.0.to_owned(), v.1))
    }
}
impl From<(u32, String)> for SpecialToken {
    fn from(v: (u32, String)) -> Self {
        Self::from((v.1, v.0))
    }
}
impl From<(u32, &str)> for SpecialToken {
    fn from(v: (u32, &str)) -> Self {
        Self::from((v.1.to_owned(), v.0))
    }
}

impl SpecialToken {
    pub fn new(id: String, ids: Vec<u32>, tokens: Vec<String>) -> Result<Self> {
        if ids.len() != tokens.len() {
            Err("SpecialToken: ids and tokens must be of the same length".into())
        } else {
            Ok(Self { id, ids, tokens })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
#[serde(transparent)]
pub struct Template(Vec<Piece>);

impl<T> TryFrom<Vec<T>> for Template
where
    T: TryInto<Piece, Error = String>,
{
    type Error = String;

    fn try_from(v: Vec<T>) -> StdResult<Self, Self::Error> {
        Ok(Self(
            v.into_iter()
                .map(|p| p.try_into())
                .collect::<StdResult<Vec<_>, Self::Error>>()?,
        ))
    }
}

impl TryFrom<String> for Template {
    type Error = String;

    fn try_from(s: String) -> StdResult<Self, Self::Error> {
        Self::try_from(s.as_ref())
    }
}

impl TryFrom<&str> for Template {
    type Error = String;

    fn try_from(s: &str) -> StdResult<Self, Self::Error> {
        Self::try_from(s.split(' ').collect::<Vec<_>>())
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Eq)]
#[serde(transparent)]
pub struct Tokens(
    #[serde(serialize_with = "crate::tokenizer::utils::ordered_map")]
    pub  AHashMap<String, SpecialToken>,
);

impl<T: Into<SpecialToken>> From<Vec<T>> for Tokens {
    fn from(v: Vec<T>) -> Self {
        Self(
            v.into_iter()
                .map(|t| {
                    let token: SpecialToken = t.into();
                    (token.id.clone(), token)
                })
                .collect(),
        )
    }
}

impl From<AHashMap<String, SpecialToken>> for Tokens {
    fn from(v: AHashMap<String, SpecialToken>) -> Self {
        Self(v)
    }
}

#[derive(Debug, Clone, PartialEq, Builder, Serialize, Deserialize, Eq)]
#[serde(tag = "type", from = "TemplateProcessingDeserializer")]
#[builder(build_fn(validate = "Self::validate"))]
pub struct TemplateProcessing {
    #[builder(try_setter, default = "\"$0\".try_into().unwrap()")]
    pub single: Template,
    #[builder(try_setter, default = "\"$A:0 $B:1\".try_into().unwrap()")]
    pair: Template,
    #[builder(setter(skip), default = "self.default_added(true)")]
    #[serde(skip)]
    added_single: usize,
    #[builder(setter(skip), default = "self.default_added(false)")]
    #[serde(skip)]
    added_pair: usize,
    #[builder(setter(into), default)]
    special_tokens: Tokens,
}

impl TemplateProcessing {
    // Getter for `single`
    pub fn get_single(&self) -> String {
        format!("{:?}", self.single)
    }

    // Setter for `single`
    pub fn set_single(&mut self, single: Template) {
        self.single = single;
    }

    // Getter for `pair`
    pub fn get_pair(&self) -> &Template {
        &self.pair
    }

    // Setter for `pair`
    pub fn set_pair(&mut self, pair: Template) {
        self.pair = pair;
    }

    // Getter for `added_single`
    pub fn get_added_single(&self) -> usize {
        self.added_single
    }

    // Setter for `added_single`
    pub fn set_added_single(&mut self, added_single: usize) {
        self.added_single = added_single;
    }

    // Getter for `added_pair`
    pub fn get_added_pair(&self) -> usize {
        self.added_pair
    }

    // Setter for `added_pair`
    pub fn set_added_pair(&mut self, added_pair: usize) {
        self.added_pair = added_pair;
    }

    // Getter for `special_tokens`
    pub fn get_special_tokens(&self) -> &Tokens {
        &self.special_tokens
    }

    // Setter for `special_tokens`
    pub fn set_special_tokens(&mut self, special_tokens: Tokens) {
        self.special_tokens = special_tokens;
    }
}

impl From<&str> for TemplateProcessingBuilderError {
    fn from(e: &str) -> Self {
        e.to_string().into()
    }
}

impl PartialEq for TemplateProcessingBuilderError {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
struct TemplateProcessingDeserializer {
    single: Template,
    pair: Template,
    special_tokens: Tokens,
}
impl From<TemplateProcessingDeserializer> for TemplateProcessing {
    fn from(t: TemplateProcessingDeserializer) -> Self {
        let added_single = count_added(&t.single, Some(&t.special_tokens));
        let added_pair = count_added(&t.pair, Some(&t.special_tokens));
        Self {
            single: t.single,
            pair: t.pair,
            added_single,
            added_pair,
            special_tokens: t.special_tokens,
        }
    }
}

fn count_added(container: &Template, special_tokens: Option<&Tokens>) -> usize {
    container
        .0
        .iter()
        .map(|p| match p {
            Piece::Sequence { .. } => 0,
            Piece::SpecialToken { id, .. } => {
                special_tokens.map_or(0, |spt| spt.0.get(id).map_or(0, |s| s.ids.len()))
            }
        })
        .sum()
}

impl TemplateProcessingBuilder {
    fn default_added(&self, is_single: bool) -> usize {
        let container = if is_single {
            self.single.as_ref()
        } else {
            self.pair.as_ref()
        };
        container.map_or(0, |pieces| {
            count_added(pieces, self.special_tokens.as_ref())
        })
    }

    fn validate(&self) -> std::result::Result<(), String> {
        let pair_has_both = self.pair.as_ref().is_none_or(|pair| {
            let mut has_a = false;
            let mut has_b = false;
            for piece in &pair.0 {
                if let Piece::Sequence {
                    id: Sequence::A, ..
                } = piece
                {
                    has_a = true;
                }
                if let Piece::Sequence {
                    id: Sequence::B, ..
                } = piece
                {
                    has_b = true;
                }
            }
            has_a && has_b
        });
        if !pair_has_both {
            return Err("Template for `pair` must use both sequences".into());
        }

        let check = |sp| {
            let exist = self
                .special_tokens
                .as_ref()
                .is_some_and(|map| map.0.contains_key(sp));

            match exist {
                false => Some(sp),
                true => None,
            }
        };

        let empty = [];
        let missing: AHashSet<&str> = self
            .single
            .as_ref()
            .map_or(empty.iter(), |s| s.0.iter())
            .chain(self.pair.as_ref().map_or(empty.iter(), |s| s.0.iter()))
            .filter_map(|piece| match piece {
                Piece::Sequence { .. } => None,
                Piece::SpecialToken { id, .. } => check(id.as_ref()),
            })
            .collect::<AHashSet<_>>();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Missing SpecialToken(s) with id(s) `{}`",
                missing.iter().join(", ")
            ))
        }
    }
}

impl Default for TemplateProcessing {
    fn default() -> Self {
        Self {
            single: "$0".try_into().unwrap(),
            pair: "$1".try_into().unwrap(),
            added_single: 0,
            added_pair: 0,
            special_tokens: Tokens::default(),
        }
    }
}

impl TemplateProcessing {
    pub fn builder() -> TemplateProcessingBuilder {
        TemplateProcessingBuilder::default()
    }

    fn apply_template(
        &self,
        template: &[Piece],
        mut encodings: Vec<Encoding>,
        add_special_tokens: bool,
    ) -> Result<Vec<Encoding>> {
        let final_encodings: Vec<Encoding> = template
            .iter()
            .flat_map(|piece| {
                match piece {
                    Piece::Sequence { id, type_id } => {
                        let i = usize::from(*id != Sequence::A);
                        let encoding = &mut encodings[i];
                        encoding.set_type_ids(vec![*type_id; encoding.len()]);
                        encoding.set_sequence_id(i);
                        Some(encoding.clone())
                    }
                    Piece::SpecialToken { id, type_id } => {
                        if add_special_tokens {
                            let tok = &self.special_tokens.0[id]; // We already checked existence above
                            let len = tok.ids.len();

                            let encoding = Encoding::new(
                                tok.ids.clone(),
                                std::iter::repeat_n(*type_id, len).collect(),
                                tok.tokens.clone(),
                                // words
                                std::iter::repeat_n(None, len).collect(),
                                // offsets
                                std::iter::repeat_n((0, 0), len).collect(),
                                // special_tokens_mask
                                std::iter::repeat_n(1, len).collect(),
                                // attention_mask
                                std::iter::repeat_n(1, len).collect(),
                                // overflowing
                                vec![],
                                // sequence_range
                                AHashMap::new(),
                            );
                            Some(encoding)
                        } else {
                            None
                        }
                    }
                }
            })
            .collect();

        //let mut pair = if encodings.len() > 1 {
        //    Some(encodings.pop().unwrap())
        //} else {
        //    None
        //};
        //let mut encoding = encodings.pop().unwrap();

        //let pair_overflowing = pair.as_mut().map_or(vec![], |e| e.take_overflowing());
        //let mut overflowing: Vec<Encoding> = encoding
        //    .take_overflowing()
        //    .iter()
        //    .map(|encoding| -> Result<Vec<Encoding>> {
        //        // 1. The pair itself
        //        let mut overflowings = self.apply_template(
        //            template,
        //            if encodings.len() > 1 {
        //                vec![encoding.clone(), encodings[1].clone()]
        //            } else {
        //                vec![encoding.clone()]
        //            },
        //            add_special_tokens,
        //        )?;

        //        // 2. Its overflowings
        //        for other_o in &pair_overflowing {
        //            overflowings.extend(self.apply_template(
        //                template,
        //                vec![encoding.clone(), other_o.clone()],
        //                add_special_tokens,
        //            )?);
        //        }

        //        Ok(overflowings)
        //    })
        //    .collect::<Result<Vec<Vec<Encoding>>>>()?
        //    .into_iter()
        //    .flatten()
        //    .collect();
        //// We also need to combine the first sequence with all other overflowings
        //overflowing.extend(
        //    pair_overflowing
        //        .into_iter()
        //        .map(|pair| {
        //            self.apply_template(template, vec![encoding.clone(), pair], add_special_tokens)
        //        })
        //        .collect::<Result<Vec<_>>>()?
        //        .into_iter()
        //        .flatten(),
        //);

        Ok(final_encodings)
    }
}

impl PostProcessor for TemplateProcessing {
    fn added_tokens(&self, is_pair: bool) -> usize {
        if is_pair {
            self.added_pair
        } else {
            self.added_single
        }
    }

    fn process_encodings(
        &self,
        encodings: Vec<Encoding>,
        add_special_tokens: bool,
    ) -> Result<Vec<Encoding>> {
        // let (encoding, pair): (Encoding, Option<Encoding>) = match encodings.len() {
        //     1 => (
        //         encodings
        //             .pop()
        //             .ok_or(ProcessorError::InvalidEncodingsVecLength)?,
        //         None,
        //     ),
        //     2 => {
        //         let pair = encodings
        //             .pop()
        //             .ok_or(ProcessorError::InvalidEncodingsVecLength)?;
        //         let encoding = encodings
        //             .pop()
        //             .ok_or(ProcessorError::InvalidEncodingsVecLength)?;
        //         (encoding, Some(pair))
        //     }
        //     _ => return Err(Box::new(ProcessorError::InvalidEncodingsVecLength)),
        // };
        let template = match encodings.len() {
            2 => &self.pair.0,
            1 => &self.single.0,
            _ => todo!(),
        };
        let encodings = self.apply_template(template, encodings, add_special_tokens)?;
        Ok(encodings)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/processors/template/tests.rs"]
mod tests;

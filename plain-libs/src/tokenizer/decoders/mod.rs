pub mod bpe;
pub mod byte_fallback;
pub mod ctc;
pub mod fuse;
pub mod sequence;
pub mod strip;
pub mod wordpiece;

// Re-export these as decoders
pub use super::pre_tokenizers::byte_level;
pub use super::pre_tokenizers::metaspace;

use serde::{Deserialize, Deserializer, Serialize};

use crate::tokenizer::decoders::bpe::BPEDecoder;
use crate::tokenizer::decoders::byte_fallback::ByteFallback;
use crate::tokenizer::decoders::ctc::CTC;
use crate::tokenizer::decoders::fuse::Fuse;
use crate::tokenizer::decoders::sequence::Sequence;
use crate::tokenizer::decoders::strip::Strip;
use crate::tokenizer::decoders::wordpiece::WordPiece;
use crate::tokenizer::normalizers::replace::Replace;
use crate::tokenizer::pre_tokenizers::byte_level::ByteLevel;
use crate::tokenizer::pre_tokenizers::metaspace::Metaspace;
use crate::tokenizer::{Decoder, Result};

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum DecoderWrapper {
    BPE(BPEDecoder),
    ByteLevel(ByteLevel),
    WordPiece(WordPiece),
    Metaspace(Metaspace),
    CTC(CTC),
    Sequence(Sequence),
    Replace(Replace),
    Fuse(Fuse),
    Strip(Strip),
    ByteFallback(ByteFallback),
}

impl<'de> Deserialize<'de> for DecoderWrapper {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        pub struct Tagged {
            #[serde(rename = "type")]
            variant: EnumType,
            #[serde(flatten)]
            rest: crate::serde_json::Value,
        }
        #[derive(Serialize, Deserialize)]
        pub enum EnumType {
            BPEDecoder,
            ByteLevel,
            WordPiece,
            Metaspace,
            CTC,
            Sequence,
            Replace,
            Fuse,
            Strip,
            ByteFallback,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum DecoderHelper {
            Tagged(Tagged),
            Legacy(crate::serde_json::Value),
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum DecoderUntagged {
            BPE(BPEDecoder),
            ByteLevel(ByteLevel),
            WordPiece(WordPiece),
            Metaspace(Metaspace),
            CTC(CTC),
            Sequence(Sequence),
            Replace(Replace),
            Fuse(Fuse),
            Strip(Strip),
            ByteFallback(ByteFallback),
        }

        let helper = DecoderHelper::deserialize(deserializer).expect("Helper");
        Ok(match helper {
            DecoderHelper::Tagged(model) => {
                let mut values: crate::serde_json::Map<String, crate::serde_json::Value> =
                    crate::serde_json::from_value(model.rest).map_err(serde::de::Error::custom)?;
                values.insert(
                    "type".to_string(),
                    crate::serde_json::to_value(&model.variant).map_err(serde::de::Error::custom)?,
                );
                let values = crate::serde_json::Value::Object(values);
                match model.variant {
                    EnumType::BPEDecoder => DecoderWrapper::BPE(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::ByteLevel => DecoderWrapper::ByteLevel(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::WordPiece => DecoderWrapper::WordPiece(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Metaspace => DecoderWrapper::Metaspace(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::CTC => DecoderWrapper::CTC(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Sequence => DecoderWrapper::Sequence(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Replace => DecoderWrapper::Replace(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Fuse => DecoderWrapper::Fuse(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Strip => DecoderWrapper::Strip(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::ByteFallback => DecoderWrapper::ByteFallback(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                }
            }
            DecoderHelper::Legacy(value) => {
                let untagged = crate::serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                match untagged {
                    DecoderUntagged::BPE(dec) => DecoderWrapper::BPE(dec),
                    DecoderUntagged::ByteLevel(dec) => DecoderWrapper::ByteLevel(dec),
                    DecoderUntagged::WordPiece(dec) => DecoderWrapper::WordPiece(dec),
                    DecoderUntagged::Metaspace(dec) => DecoderWrapper::Metaspace(dec),
                    DecoderUntagged::CTC(dec) => DecoderWrapper::CTC(dec),
                    DecoderUntagged::Sequence(dec) => DecoderWrapper::Sequence(dec),
                    DecoderUntagged::Replace(dec) => DecoderWrapper::Replace(dec),
                    DecoderUntagged::Fuse(dec) => DecoderWrapper::Fuse(dec),
                    DecoderUntagged::Strip(dec) => DecoderWrapper::Strip(dec),
                    DecoderUntagged::ByteFallback(dec) => DecoderWrapper::ByteFallback(dec),
                }
            }
        })
    }
}

impl Decoder for DecoderWrapper {
    fn decode_chain(&self, tokens: Vec<String>) -> Result<Vec<String>> {
        match self {
            Self::BPE(bpe) => bpe.decode_chain(tokens),
            Self::ByteLevel(bl) => bl.decode_chain(tokens),
            Self::Metaspace(ms) => ms.decode_chain(tokens),
            Self::WordPiece(wp) => wp.decode_chain(tokens),
            Self::CTC(ctc) => ctc.decode_chain(tokens),
            Self::Sequence(seq) => seq.decode_chain(tokens),
            Self::Replace(seq) => seq.decode_chain(tokens),
            Self::ByteFallback(bf) => bf.decode_chain(tokens),
            Self::Strip(bf) => bf.decode_chain(tokens),
            Self::Fuse(bf) => bf.decode_chain(tokens),
        }
    }
}

impl_enum_from!(BPEDecoder, DecoderWrapper, BPE);
impl_enum_from!(ByteLevel, DecoderWrapper, ByteLevel);
impl_enum_from!(ByteFallback, DecoderWrapper, ByteFallback);
impl_enum_from!(Fuse, DecoderWrapper, Fuse);
impl_enum_from!(Strip, DecoderWrapper, Strip);
impl_enum_from!(Metaspace, DecoderWrapper, Metaspace);
impl_enum_from!(WordPiece, DecoderWrapper, WordPiece);
impl_enum_from!(CTC, DecoderWrapper, CTC);
impl_enum_from!(Sequence, DecoderWrapper, Sequence);
impl_enum_from!(Replace, DecoderWrapper, Replace);

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/decoders/mod/tests.rs"]
mod tests;

pub mod bert;
pub mod byte_level;
pub mod precompiled;
pub mod prepend;
pub mod replace;
pub mod strip;
pub mod unicode;
pub mod utils;
pub use crate::tokenizer::normalizers::bert::BertNormalizer;
pub use crate::tokenizer::normalizers::byte_level::ByteLevel;
pub use crate::tokenizer::normalizers::precompiled::Precompiled;
pub use crate::tokenizer::normalizers::prepend::Prepend;
pub use crate::tokenizer::normalizers::replace::Replace;
pub use crate::tokenizer::normalizers::strip::{Strip, StripAccents};
pub use crate::tokenizer::normalizers::unicode::{NFC, NFD, NFKC, NFKD, Nmt};
pub use crate::tokenizer::normalizers::utils::{Lowercase, Sequence};
use serde::{Deserialize, Deserializer, Serialize};

use crate::tokenizer::{NormalizedString, Normalizer};

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum NormalizerWrapper {
    BertNormalizer(BertNormalizer),
    StripNormalizer(Strip),
    StripAccents(StripAccents),
    NFC(NFC),
    NFD(NFD),
    NFKC(NFKC),
    NFKD(NFKD),
    Sequence(Sequence),
    Lowercase(Lowercase),
    Nmt(Nmt),
    Precompiled(Precompiled),
    Replace(Replace),
    Prepend(Prepend),
    ByteLevel(ByteLevel),
}

impl<'de> Deserialize<'de> for NormalizerWrapper {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        pub struct Tagged {
            #[serde(rename = "type")]
            variant: EnumType,
            #[serde(flatten)]
            rest: crate::serde_json::Value,
        }
        #[derive(Debug, Serialize, Deserialize)]
        pub enum EnumType {
            Bert,
            Strip,
            StripAccents,
            NFC,
            NFD,
            NFKC,
            NFKD,
            Sequence,
            Lowercase,
            Nmt,
            Precompiled,
            Replace,
            Prepend,
            ByteLevel,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum NormalizerHelper {
            Tagged(Tagged),
            Legacy(crate::serde_json::Value),
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum NormalizerUntagged {
            BertNormalizer(BertNormalizer),
            StripNormalizer(Strip),
            StripAccents(StripAccents),
            NFC(NFC),
            NFD(NFD),
            NFKC(NFKC),
            NFKD(NFKD),
            Sequence(Sequence),
            Lowercase(Lowercase),
            Nmt(Nmt),
            Precompiled(Precompiled),
            Replace(Replace),
            Prepend(Prepend),
            ByteLevel(ByteLevel),
        }

        let helper = NormalizerHelper::deserialize(deserializer)?;
        Ok(match helper {
            NormalizerHelper::Tagged(model) => {
                let mut values: crate::serde_json::Map<String, crate::serde_json::Value> =
                    crate::serde_json::from_value(model.rest).expect("Parsed values");
                values.insert(
                    "type".to_string(),
                    crate::serde_json::to_value(&model.variant).expect("Reinsert"),
                );
                let values = crate::serde_json::Value::Object(values);
                match model.variant {
                    EnumType::Bert => NormalizerWrapper::BertNormalizer(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Strip => NormalizerWrapper::StripNormalizer(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::StripAccents => NormalizerWrapper::StripAccents(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::NFC => NormalizerWrapper::NFC(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::NFD => NormalizerWrapper::NFD(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::NFKC => NormalizerWrapper::NFKC(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::NFKD => NormalizerWrapper::NFKD(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Sequence => NormalizerWrapper::Sequence(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Lowercase => NormalizerWrapper::Lowercase(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Nmt => NormalizerWrapper::Nmt(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Precompiled => NormalizerWrapper::Precompiled(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Replace => NormalizerWrapper::Replace(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Prepend => NormalizerWrapper::Prepend(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::ByteLevel => NormalizerWrapper::ByteLevel(
                        crate::serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                }
            }

            NormalizerHelper::Legacy(value) => {
                let untagged = crate::serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                match untagged {
                    NormalizerUntagged::BertNormalizer(bpe) => {
                        NormalizerWrapper::BertNormalizer(bpe)
                    }
                    NormalizerUntagged::StripNormalizer(bpe) => {
                        NormalizerWrapper::StripNormalizer(bpe)
                    }
                    NormalizerUntagged::StripAccents(bpe) => NormalizerWrapper::StripAccents(bpe),
                    NormalizerUntagged::NFC(bpe) => NormalizerWrapper::NFC(bpe),
                    NormalizerUntagged::NFD(bpe) => NormalizerWrapper::NFD(bpe),
                    NormalizerUntagged::NFKC(bpe) => NormalizerWrapper::NFKC(bpe),
                    NormalizerUntagged::NFKD(bpe) => NormalizerWrapper::NFKD(bpe),
                    NormalizerUntagged::Sequence(seq) => NormalizerWrapper::Sequence(seq),
                    NormalizerUntagged::Lowercase(bpe) => NormalizerWrapper::Lowercase(bpe),
                    NormalizerUntagged::Nmt(bpe) => NormalizerWrapper::Nmt(bpe),
                    NormalizerUntagged::Precompiled(bpe) => NormalizerWrapper::Precompiled(bpe),
                    NormalizerUntagged::Replace(bpe) => NormalizerWrapper::Replace(bpe),
                    NormalizerUntagged::Prepend(bpe) => NormalizerWrapper::Prepend(bpe),
                    NormalizerUntagged::ByteLevel(bpe) => NormalizerWrapper::ByteLevel(bpe),
                }
            }
        })
    }
}

impl Normalizer for NormalizerWrapper {
    fn normalize(&self, normalized: &mut NormalizedString) -> crate::tokenizer::Result<()> {
        match self {
            Self::BertNormalizer(bn) => bn.normalize(normalized),
            Self::StripNormalizer(sn) => sn.normalize(normalized),
            Self::StripAccents(sn) => sn.normalize(normalized),
            Self::NFC(nfc) => nfc.normalize(normalized),
            Self::NFD(nfd) => nfd.normalize(normalized),
            Self::NFKC(nfkc) => nfkc.normalize(normalized),
            Self::NFKD(nfkd) => nfkd.normalize(normalized),
            Self::Sequence(sequence) => sequence.normalize(normalized),
            Self::Lowercase(lc) => lc.normalize(normalized),
            Self::Nmt(lc) => lc.normalize(normalized),
            Self::Precompiled(lc) => lc.normalize(normalized),
            Self::Replace(lc) => lc.normalize(normalized),
            Self::Prepend(lc) => lc.normalize(normalized),
            Self::ByteLevel(lc) => lc.normalize(normalized),
        }
    }
}

impl_enum_from!(BertNormalizer, NormalizerWrapper, BertNormalizer);
impl_enum_from!(NFKD, NormalizerWrapper, NFKD);
impl_enum_from!(NFKC, NormalizerWrapper, NFKC);
impl_enum_from!(NFC, NormalizerWrapper, NFC);
impl_enum_from!(NFD, NormalizerWrapper, NFD);
impl_enum_from!(Strip, NormalizerWrapper, StripNormalizer);
impl_enum_from!(StripAccents, NormalizerWrapper, StripAccents);
impl_enum_from!(Sequence, NormalizerWrapper, Sequence);
impl_enum_from!(Lowercase, NormalizerWrapper, Lowercase);
impl_enum_from!(Nmt, NormalizerWrapper, Nmt);
impl_enum_from!(Precompiled, NormalizerWrapper, Precompiled);
impl_enum_from!(Replace, NormalizerWrapper, Replace);
impl_enum_from!(Prepend, NormalizerWrapper, Prepend);
impl_enum_from!(ByteLevel, NormalizerWrapper, ByteLevel);

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/normalizers/mod/tests.rs"]
mod tests;

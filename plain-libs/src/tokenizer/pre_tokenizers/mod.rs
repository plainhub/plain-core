pub mod bert;
pub mod byte_level;
pub mod delimiter;
pub mod digits;
pub mod fixed_length;
pub mod metaspace;
pub mod punctuation;
pub mod sequence;
pub mod split;
pub mod unicode_scripts;
pub mod whitespace;

use serde::{Deserialize, Deserializer, Serialize};

use crate::tokenizer::pre_tokenizers::bert::BertPreTokenizer;
use crate::tokenizer::pre_tokenizers::byte_level::ByteLevel;
use crate::tokenizer::pre_tokenizers::delimiter::CharDelimiterSplit;
use crate::tokenizer::pre_tokenizers::digits::Digits;
use crate::tokenizer::pre_tokenizers::fixed_length::FixedLength;
use crate::tokenizer::pre_tokenizers::metaspace::Metaspace;
use crate::tokenizer::pre_tokenizers::punctuation::Punctuation;
use crate::tokenizer::pre_tokenizers::sequence::Sequence;
use crate::tokenizer::pre_tokenizers::split::Split;
use crate::tokenizer::pre_tokenizers::unicode_scripts::UnicodeScripts;
use crate::tokenizer::pre_tokenizers::whitespace::{Whitespace, WhitespaceSplit};
use crate::tokenizer::{PreTokenizedString, PreTokenizer};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum PreTokenizerWrapper {
    BertPreTokenizer(BertPreTokenizer),
    ByteLevel(ByteLevel),
    Delimiter(CharDelimiterSplit),
    Metaspace(Metaspace),
    Whitespace(Whitespace),
    Sequence(Sequence),
    Split(Split),
    Punctuation(Punctuation),
    WhitespaceSplit(WhitespaceSplit),
    Digits(Digits),
    UnicodeScripts(UnicodeScripts),
    FixedLength(FixedLength),
}

impl PreTokenizer for PreTokenizerWrapper {
    fn pre_tokenize(&self, normalized: &mut PreTokenizedString) -> crate::tokenizer::Result<()> {
        match self {
            Self::BertPreTokenizer(bpt) => bpt.pre_tokenize(normalized),
            Self::ByteLevel(bpt) => bpt.pre_tokenize(normalized),
            Self::Delimiter(dpt) => dpt.pre_tokenize(normalized),
            Self::Metaspace(mspt) => mspt.pre_tokenize(normalized),
            Self::Whitespace(wspt) => wspt.pre_tokenize(normalized),
            Self::Punctuation(tok) => tok.pre_tokenize(normalized),
            Self::Sequence(tok) => tok.pre_tokenize(normalized),
            Self::Split(tok) => tok.pre_tokenize(normalized),
            Self::WhitespaceSplit(wspt) => wspt.pre_tokenize(normalized),
            Self::Digits(wspt) => wspt.pre_tokenize(normalized),
            Self::UnicodeScripts(us) => us.pre_tokenize(normalized),
            Self::FixedLength(fl) => fl.pre_tokenize(normalized),
        }
    }
}

impl<'de> Deserialize<'de> for PreTokenizerWrapper {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        pub struct Tagged {
            #[serde(rename = "type")]
            variant: EnumType,
            #[serde(flatten)]
            rest: serde_json::Value,
        }
        #[derive(Deserialize, Serialize)]
        pub enum EnumType {
            BertPreTokenizer,
            ByteLevel,
            Delimiter,
            Metaspace,
            Whitespace,
            Sequence,
            Split,
            Punctuation,
            WhitespaceSplit,
            Digits,
            UnicodeScripts,
            FixedLength,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum PreTokenizerHelper {
            Tagged(Tagged),
            Legacy(serde_json::Value),
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum PreTokenizerUntagged {
            BertPreTokenizer(BertPreTokenizer),
            ByteLevel(ByteLevel),
            Delimiter(CharDelimiterSplit),
            Metaspace(Metaspace),
            Whitespace(Whitespace),
            Sequence(Sequence),
            Split(Split),
            Punctuation(Punctuation),
            WhitespaceSplit(WhitespaceSplit),
            Digits(Digits),
            UnicodeScripts(UnicodeScripts),
            FixedLength(FixedLength),
        }

        let helper = PreTokenizerHelper::deserialize(deserializer)?;

        Ok(match helper {
            PreTokenizerHelper::Tagged(pretok) => {
                let mut values: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_value(pretok.rest).map_err(serde::de::Error::custom)?;
                values.insert(
                    "type".to_string(),
                    serde_json::to_value(&pretok.variant).map_err(serde::de::Error::custom)?,
                );
                let values = serde_json::Value::Object(values);
                match pretok.variant {
                    EnumType::BertPreTokenizer => PreTokenizerWrapper::BertPreTokenizer(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::ByteLevel => PreTokenizerWrapper::ByteLevel(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Delimiter => PreTokenizerWrapper::Delimiter(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Metaspace => PreTokenizerWrapper::Metaspace(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Whitespace => PreTokenizerWrapper::Whitespace(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Sequence => PreTokenizerWrapper::Sequence(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Split => PreTokenizerWrapper::Split(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Punctuation => PreTokenizerWrapper::Punctuation(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::WhitespaceSplit => PreTokenizerWrapper::WhitespaceSplit(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::Digits => PreTokenizerWrapper::Digits(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::UnicodeScripts => PreTokenizerWrapper::UnicodeScripts(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                    EnumType::FixedLength => PreTokenizerWrapper::FixedLength(
                        serde_json::from_value(values).map_err(serde::de::Error::custom)?,
                    ),
                }
            }

            PreTokenizerHelper::Legacy(value) => {
                let untagged = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                match untagged {
                    PreTokenizerUntagged::BertPreTokenizer(bert) => {
                        PreTokenizerWrapper::BertPreTokenizer(bert)
                    }
                    PreTokenizerUntagged::ByteLevel(byte_level) => {
                        PreTokenizerWrapper::ByteLevel(byte_level)
                    }
                    PreTokenizerUntagged::Delimiter(delimiter) => {
                        PreTokenizerWrapper::Delimiter(delimiter)
                    }
                    PreTokenizerUntagged::Metaspace(metaspace) => {
                        PreTokenizerWrapper::Metaspace(metaspace)
                    }
                    PreTokenizerUntagged::Whitespace(whitespace) => {
                        PreTokenizerWrapper::Whitespace(whitespace)
                    }
                    PreTokenizerUntagged::Sequence(sequence) => {
                        PreTokenizerWrapper::Sequence(sequence)
                    }
                    PreTokenizerUntagged::Split(split) => PreTokenizerWrapper::Split(split),
                    PreTokenizerUntagged::Punctuation(punctuation) => {
                        PreTokenizerWrapper::Punctuation(punctuation)
                    }
                    PreTokenizerUntagged::WhitespaceSplit(whitespace_split) => {
                        PreTokenizerWrapper::WhitespaceSplit(whitespace_split)
                    }
                    PreTokenizerUntagged::Digits(digits) => PreTokenizerWrapper::Digits(digits),
                    PreTokenizerUntagged::UnicodeScripts(unicode_scripts) => {
                        PreTokenizerWrapper::UnicodeScripts(unicode_scripts)
                    }
                    PreTokenizerUntagged::FixedLength(fixed_length) => {
                        PreTokenizerWrapper::FixedLength(fixed_length)
                    }
                }
            }
        })
    }
}

impl_enum_from!(BertPreTokenizer, PreTokenizerWrapper, BertPreTokenizer);
impl_enum_from!(ByteLevel, PreTokenizerWrapper, ByteLevel);
impl_enum_from!(CharDelimiterSplit, PreTokenizerWrapper, Delimiter);
impl_enum_from!(Whitespace, PreTokenizerWrapper, Whitespace);
impl_enum_from!(Punctuation, PreTokenizerWrapper, Punctuation);
impl_enum_from!(Sequence, PreTokenizerWrapper, Sequence);
impl_enum_from!(Split, PreTokenizerWrapper, Split);
impl_enum_from!(Metaspace, PreTokenizerWrapper, Metaspace);
impl_enum_from!(WhitespaceSplit, PreTokenizerWrapper, WhitespaceSplit);
impl_enum_from!(Digits, PreTokenizerWrapper, Digits);
impl_enum_from!(UnicodeScripts, PreTokenizerWrapper, UnicodeScripts);
impl_enum_from!(FixedLength, PreTokenizerWrapper, FixedLength);

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/pre_tokenizers/mod/tests.rs"]
mod tests;

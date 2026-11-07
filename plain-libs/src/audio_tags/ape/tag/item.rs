use crate::audio_tags::ape::constants::INVALID_KEYS;
use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::macros::decode_err;
use crate::audio_tags::tag::item::ItemValueRef;
use crate::audio_tags::tag::{ItemValue, TagItem, TagType};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ApeItem {
    pub read_only: bool,
    pub(crate) key: String,
    pub(crate) value: ItemValue,
}

impl ApeItem {
    pub fn new(key: String, value: ItemValue) -> Result<Self> {
        if INVALID_KEYS.contains(&&*key.to_uppercase()) {
            decode_err!(@BAIL Ape, "APE tag item contains an illegal key");
        }

        if !(2..=255).contains(&key.len()) {
            decode_err!(@BAIL Ape, "APE tag item key has an invalid length (< 2 || > 255)");
        }

        if key.chars().any(|c| !(0x20..=0x7E).contains(&(c as u32))) {
            decode_err!(@BAIL Ape, "APE tag item key contains invalid characters");
        }

        Ok(Self {
            read_only: false,
            key,
            value,
        })
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &ItemValue {
        &self.value
    }

    // Used internally, has no correctness checks
    pub(crate) fn text(key: &str, value: String) -> Self {
        Self {
            read_only: false,
            key: String::from(key),
            value: ItemValue::Text(value),
        }
    }
}

impl TryFrom<TagItem> for ApeItem {
    type Error = LoftyError;

    fn try_from(value: TagItem) -> std::result::Result<Self, Self::Error> {
        Self::new(
            value
                .item_key
                .map_key(TagType::Ape, false)
                .ok_or_else(|| decode_err!(Ape, "Attempted to convert an unsupported item key"))?
                .to_string(),
            value.item_value,
        )
    }
}

pub(crate) struct ApeItemRef<'a> {
    pub read_only: bool,
    pub key: &'a str,
    pub value: ItemValueRef<'a>,
}

impl<'a> Into<ApeItemRef<'a>> for &'a ApeItem {
    fn into(self) -> ApeItemRef<'a> {
        ApeItemRef {
            read_only: self.read_only,
            key: self.key(),
            value: (&self.value).into(),
        }
    }
}

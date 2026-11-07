use super::{super::OrderedVocabIter, WordLevel, WordLevelBuilder};
use ahash::AHashSet;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, Visitor},
    ser::SerializeStruct,
};

impl Serialize for WordLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut model = serializer.serialize_struct("WordLevel", 3)?;
        let ordered_vocab = OrderedVocabIter::new(&self.vocab_r);
        model.serialize_field("type", "WordLevel")?;
        model.serialize_field("vocab", &ordered_vocab)?;
        model.serialize_field("unk_token", &self.unk_token)?;
        model.end()
    }
}

impl<'de> Deserialize<'de> for WordLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "WordLevel",
            &["type", "vocab", "unk_token"],
            WordLevelVisitor,
        )
    }
}

struct WordLevelVisitor;
impl<'de> Visitor<'de> for WordLevelVisitor {
    type Value = WordLevel;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "struct WordLevel")
    }

    fn visit_map<V>(self, mut map: V) -> std::result::Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut builder = WordLevelBuilder::new();
        let mut missing_fields = vec![
            // for retrocompatibility the "type" field is not mandatory
            "unk_token",
            "vocab",
        ]
        .into_iter()
        .collect::<AHashSet<_>>();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_ref() {
                "vocab" => builder = builder.vocab(map.next_value()?),
                "unk_token" => builder = builder.unk_token(map.next_value()?),
                "type" => match map.next_value()? {
                    "WordLevel" => {}
                    u => {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(u),
                            &"WordLevel",
                        ));
                    }
                },
                _ => {}
            }
            missing_fields.remove::<str>(&key);
        }

        if !missing_fields.is_empty() {
            Err(serde::de::Error::missing_field(
                missing_fields.iter().next().unwrap(),
            ))
        } else {
            Ok(builder.build().map_err(serde::de::Error::custom)?)
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/wordlevel/serialization/tests.rs"]
mod tests;

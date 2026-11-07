use super::model::Unigram;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, MapAccess, Visitor},
    ser::SerializeStruct,
};

impl Serialize for Unigram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut model = serializer.serialize_struct("Unigram", 3)?;

        model.serialize_field("type", "Unigram")?;
        model.serialize_field("unk_id", &self.unk_id)?;
        model.serialize_field("vocab", &self.vocab)?;
        model.serialize_field("byte_fallback", &self.byte_fallback())?;

        model.end()
    }
}

impl<'de> Deserialize<'de> for Unigram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "Unigram",
            &["type", "vocab", "unk_id", "byte_fallback"],
            UnigramVisitor,
        )
    }
}

struct UnigramVisitor;
impl<'de> Visitor<'de> for UnigramVisitor {
    type Value = Unigram;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "struct Unigram")
    }

    fn visit_map<V>(self, mut map: V) -> std::result::Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut vocab: Option<Vec<(String, f64)>> = None;
        let mut unk_id: Option<usize> = None;
        let mut byte_fallback: bool = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_ref() {
                "unk_id" => {
                    unk_id = map.next_value()?;
                }
                "byte_fallback" => byte_fallback = map.next_value()?,
                "vocab" => vocab = Some(map.next_value()?),
                "type" => match map.next_value()? {
                    "Unigram" => {}
                    u => {
                        return Err(serde::de::Error::invalid_value(
                            serde::de::Unexpected::Str(u),
                            &"Unigram",
                        ));
                    }
                },
                _ => (),
            }
        }
        match (vocab, unk_id, byte_fallback) {
            (Some(vocab), unk_id, byte_fallback) => Ok(Unigram::from(vocab, unk_id, byte_fallback)
                .map_err(|err| Error::custom(format!("Unable to load vocab {err:?}")))?),
            (None, _, _) => Err(Error::custom("Missing vocab")),
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/unigram/serialization/test.rs"]
mod test;

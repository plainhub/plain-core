use std::collections::HashMap;
pub(crate) mod cache;

#[cfg(feature = "ai-image-search")]
mod fancy_engine;
#[cfg(feature = "ai-image-search")]
pub use fancy_engine::SysRegex;

pub mod padding;
pub mod truncation;

use serde::{Serialize, Serializer};
use std::collections::BTreeMap;

#[derive(serde::Deserialize, Serialize)]
pub(super) struct ComponentTag<T> {
    pub(super) r#type: T,
}

pub(crate) fn ordered_map<S, K, V>(
    value: &HashMap<K, V>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
    K: Serialize + std::cmp::Ord,
    V: Serialize,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}

macro_rules! impl_enum_from (
    ($from_ty:ty, $enum:ty, $variant:ident) => {
        impl From<$from_ty> for $enum {
            fn from(from: $from_ty) -> Self {
                <$enum>::$variant(from)
            }
        }
    }
);

macro_rules! impl_serde_type{
    (
     $(#[$meta:meta])*
     $vis:vis struct $struct_name:ident {
        $(
        $(#[$field_meta:meta])*
        $field_vis:vis $field_name:ident : $field_type:ty
        ),*$(,)+
    }
    ) => {
        $(#[$meta])*
        #[derive(Serialize)]
        #[serde(tag = "type")]
        $vis struct $struct_name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_type,
            )*
        }

        impl<'de> serde::Deserialize<'de> for $struct_name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
                #[derive(serde::Deserialize)]
                enum ComponentType {
                    $struct_name,
                }

                #[derive(serde::Deserialize)]
                struct Fields {
                    $(
                        $(#[$field_meta])*
                        $field_name: $field_type,
                    )*
                }

                #[derive(serde::Deserialize)]
                struct Component {
                    #[serde(rename = "type")]
                    _type: ComponentType,
                    #[serde(flatten)]
                    fields: Fields,
                }

                let Component { fields, .. } = serde::Deserialize::deserialize(deserializer)?;
                Ok(Self { $($field_name: fields.$field_name,)* })
            }
        }
    };
    (
     $(#[$meta:meta])*
     $vis:vis struct $struct_name:ident;
    ) => {
        $(#[$meta])*
        $vis struct $struct_name;

        impl serde::Serialize for $struct_name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let tag = $crate::tokenizer::utils::ComponentTag {
                    r#type: stringify!($struct_name),
                };
                serde::Serialize::serialize(&tag, serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $struct_name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let tag: $crate::tokenizer::utils::ComponentTag<String> =
                    serde::Deserialize::deserialize(deserializer)?;
                if tag.r#type != stringify!($struct_name) {
                    return Err(serde::de::Error::unknown_variant(
                        &tag.r#type,
                        &[stringify!($struct_name)],
                    ));
                }
                Ok($struct_name)
            }
        }
    }
}

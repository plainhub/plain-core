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
        paste::paste!{
            $(#[$meta])*
            #[derive(Serialize, Deserialize)]
            #[serde(tag = "type", from = $struct_name "Deserializer")]
            $vis struct $struct_name{
                $(
                    $(#[$field_meta])*
                    $field_vis $field_name : $field_type,
                )*
            }

            $(#[$meta])*
            #[derive(Deserialize)]
            #[serde(tag = "type", remote = $struct_name "")]
            struct [<$struct_name Def>]{
                $(
                    $(#[$field_meta])*
                    $field_vis $field_name : $field_type,
                )*
            }

            #[derive(Deserialize)]
            enum [<$struct_name Type>] {
                $struct_name,
            }

            #[derive(Deserialize)]
            struct [<$struct_name Deserializer>] {

                r#type: [<$struct_name Type>],
                #[serde(flatten, with = $struct_name "Def")]
                r#struct: $struct_name,
            }

            impl std::convert::From<[<$struct_name Deserializer>]> for $struct_name {
                fn from(v: [<$struct_name Deserializer>]) -> Self {
                    v.r#struct
                }
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

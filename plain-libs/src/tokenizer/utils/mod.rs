pub(crate) mod cache;

#[cfg(feature = "ai-image-search")]
mod fancy;
#[cfg(feature = "ai-image-search")]
pub use fancy::SysRegex;

#[cfg(not(feature = "ai-image-search"))]
compile_error!("The `fancy-regex` feature must be enabled");

pub mod padding;
pub mod truncation;

use ahash::AHashMap;
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;

pub(crate) fn ordered_map<S, K, V>(
    value: &AHashMap<K, V>,
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

#[macro_export]
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
        paste::paste!{
            $(#[$meta])*
            $vis struct $struct_name;

            impl serde::Serialize for $struct_name {
                fn serialize<S>(&self, serializer: S)  -> std::result::Result<S::Ok, S::Error> where
                    S: serde::ser::Serializer {
                    let helper = [<$struct_name Helper>]{r#type: [<$struct_name Type>]::$struct_name};
                    helper.serialize(serializer)
                }
            }

            impl<'de> serde::Deserialize<'de> for $struct_name {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    let _helper = [<$struct_name Helper>]::deserialize(deserializer)?;
                    Ok($struct_name)
                }
            }

            #[derive(serde::Serialize, serde::Deserialize)]
            enum [<$struct_name Type>] {
                $struct_name,
            }

            #[derive(serde::Serialize, serde::Deserialize)]
            struct [<$struct_name Helper>] {

                r#type: [<$struct_name Type>],
            }
        }
    }
}

// Re-export macro_rules_attribute
pub use macro_rules_attribute::macro_rules_attribute;

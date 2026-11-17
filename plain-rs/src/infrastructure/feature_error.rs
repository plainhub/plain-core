#[macro_export]
macro_rules! feature_error {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire),+ } }
            pub fn from_error(error: &plain_libs::anyhow::Error) -> Option<Self> {
                error.chain().find_map(|cause| cause.downcast_ref::<Self>()).copied()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
        }
        impl std::error::Error for $name {}
        impl std::str::FromStr for $name {
            type Err = serde::de::value::Error;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                <Self as serde::Deserialize>::deserialize(serde::de::value::StrDeserializer::new(value))
            }
        }
        #[cfg(feature = "library")]
        impl From<$name> for $crate::infrastructure::error::DomainError {
            fn from(code: $name) -> Self { Self::Code(Box::new(code)) }
        }
    };
}

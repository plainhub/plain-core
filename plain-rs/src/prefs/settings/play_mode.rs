#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[cfg_attr(feature = "graphql", graphql(rename_items = "SCREAMING_SNAKE_CASE"))]
pub enum MediaPlayMode {
    Repeat,
    RepeatOne,
    Shuffle,
}
impl<'de> serde::Deserialize<'de> for MediaPlayMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        super::enum_value::deserialize(
            deserializer,
            &[
                (0, "REPEAT", Self::Repeat),
                (1, "REPEAT_ONE", Self::RepeatOne),
                (2, "SHUFFLE", Self::Shuffle),
            ],
        )
    }
}
impl MediaPlayMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Repeat => "REPEAT",
            Self::RepeatOne => "REPEAT_ONE",
            Self::Shuffle => "SHUFFLE",
        }
    }
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "REPEAT" => Some(Self::Repeat),
            "REPEAT_ONE" => Some(Self::RepeatOne),
            "SHUFFLE" => Some(Self::Shuffle),
            _ => None,
        }
    }
}

#[cfg(all(test, feature = "library"))]
#[path = "../../../tests/unit/prefs/settings/play_mode.rs"]
mod tests;

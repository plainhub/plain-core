//! Wire enums shared by every GraphQL host — the desktop `api` stack and
//! the `media_gql` roots — mirroring plain-app's shared `enums` package.
//! One copy per wire name; hosts must never define a second one or the
//! merged schema panics at build.

/// Taggable data domains — the plain-app `DataType` wire enum
/// (`shared/src/commonMain/kotlin/com/ismartcoding/plain/enums/DataType.kt`).
/// GraphQL-arg and tag-column domain; `kind()` is the ordinal stored in
/// the shared library db `type` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
#[cfg_attr(
    feature = "graphql",
    graphql(name = "DataType", rename_items = "SCREAMING_SNAKE_CASE")
)]
pub enum DataType {
    Default,
    Audio,
    Video,
    Image,
    Sms,
    Contact,
    Note,
    FeedEntry,
    Call,
    Package,
    File,
    AppFile,
    Doc,
}

impl DataType {
    /// Numeric plain-app `DataType` ordinal — the tag's stored `type`
    /// column.
    pub fn kind(self) -> i32 {
        match self {
            Self::Default => 0,
            Self::Audio => 1,
            Self::Video => 2,
            Self::Image => 3,
            Self::Sms => 4,
            Self::Contact => 5,
            Self::Note => 6,
            Self::FeedEntry => 7,
            Self::Call => 8,
            Self::Package => 21,
            Self::File => 22,
            Self::AppFile => 23,
            Self::Doc => 24,
        }
    }

    /// Convert an existing numeric storage kind to its domain type.
    pub fn from_kind(kind: i32) -> Option<Self> {
        [
            Self::Default, Self::Audio, Self::Video, Self::Image, Self::Sms, Self::Contact,
            Self::Note, Self::FeedEntry, Self::Call, Self::Package, Self::File, Self::AppFile,
            Self::Doc,
        ]
        .into_iter()
        .find(|value| value.kind() == kind)
    }

    /// Media index type filter — "audio" / "video" / "image" / "doc";
    /// `None` for domains the media index does not cover.
    pub fn media_type_str(self) -> Option<&'static str> {
        match self {
            Self::Audio => Some("audio"),
            Self::Video => Some("video"),
            Self::Image => Some("image"),
            Self::Doc => Some("doc"),
            _ => None,
        }
    }
}

/// Media-library item kinds accepted by media item actions and buckets
#[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
#[allow(non_camel_case_types)] // plain-app SDL contract names
pub enum MediaDataType {
    AUDIO,
    VIDEO,
    IMAGE,
    DOC,
}

impl MediaDataType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AUDIO => "AUDIO",
            Self::VIDEO => "VIDEO",
            Self::IMAGE => "IMAGE",
            Self::DOC => "DOC",
        }
    }

    pub fn from_data_type(kind: DataType) -> Option<Self> {
        match kind {
            DataType::Audio => Some(Self::AUDIO),
            DataType::Video => Some(Self::VIDEO),
            DataType::Image => Some(Self::IMAGE),
            DataType::Doc => Some(Self::DOC),
            _ => None,
        }
    }

    pub fn data_type(self) -> DataType {
        match self {
            MediaDataType::DOC => DataType::Doc,
            MediaDataType::AUDIO => DataType::Audio,
            MediaDataType::VIDEO => DataType::Video,
            MediaDataType::IMAGE => DataType::Image,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/enums.rs"]
mod tests;

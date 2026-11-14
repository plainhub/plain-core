#[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaSortBy {
    UpdatedAtAsc,
    UpdatedAtDesc,
    SizeAsc,
    SizeDesc,
    NameAsc,
    NameDesc,
    TitleAsc,
    TitleDesc,
    TakenAtDesc,
}

impl MediaSortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpdatedAtAsc => "UPDATED_AT_ASC",
            Self::UpdatedAtDesc => "UPDATED_AT_DESC",
            Self::SizeAsc => "SIZE_ASC",
            Self::SizeDesc => "SIZE_DESC",
            Self::NameAsc => "NAME_ASC",
            Self::NameDesc => "NAME_DESC",
            Self::TitleAsc => "TITLE_ASC",
            Self::TitleDesc => "TITLE_DESC",
            Self::TakenAtDesc => "TAKEN_AT_DESC",
        }
    }
}

macro_rules! sort_enum {
    ($name:ident, $asc:ident, $desc:ident $(, $taken:ident)?) => {
        #[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize)]
        #[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name {
            UpdatedAtAsc,
            UpdatedAtDesc,
            SizeAsc,
            SizeDesc,
            $asc,
            $desc,
            $($taken,)?
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                super::enum_value::deserialize(deserializer, &[
                    (0, "UPDATED_AT_ASC", Self::UpdatedAtAsc),
                    (1, "UPDATED_AT_DESC", Self::UpdatedAtDesc),
                    (2, "SIZE_ASC", Self::SizeAsc),
                    (3, "SIZE_DESC", Self::SizeDesc),
                    (4, MediaSortBy::$asc.as_str(), Self::$asc),
                    (5, MediaSortBy::$desc.as_str(), Self::$desc),
                    $((6, MediaSortBy::$taken.as_str(), Self::$taken),)?
                ])
            }
        }
        impl $name {
            pub fn as_str(self) -> &'static str {
                MediaSortBy::from(self).as_str()
            }
        }
        impl From<$name> for MediaSortBy {
            fn from(value: $name) -> Self {
                match value {
                    $name::UpdatedAtAsc => Self::UpdatedAtAsc,
                    $name::UpdatedAtDesc => Self::UpdatedAtDesc,
                    $name::SizeAsc => Self::SizeAsc,
                    $name::SizeDesc => Self::SizeDesc,
                    $name::$asc => Self::$asc,
                    $name::$desc => Self::$desc,
                    $($name::$taken => Self::$taken,)?
                }
            }
        }
    };
}

sort_enum!(FileSortBy, NameAsc, NameDesc);
sort_enum!(ImageSortBy, NameAsc, NameDesc, TakenAtDesc);
sort_enum!(VideoSortBy, NameAsc, NameDesc, TakenAtDesc);
sort_enum!(DocSortBy, NameAsc, NameDesc);
sort_enum!(AudioSortBy, TitleAsc, TitleDesc);

#[cfg(feature = "filesystem")]
impl From<FileSortBy> for plain_libs::filesystem::SortBy {
    fn from(value: FileSortBy) -> Self {
        match value {
            FileSortBy::UpdatedAtAsc => Self::DateAsc,
            FileSortBy::UpdatedAtDesc => Self::DateDesc,
            FileSortBy::SizeAsc => Self::SizeAsc,
            FileSortBy::SizeDesc => Self::SizeDesc,
            FileSortBy::NameAsc => Self::NameAsc,
            FileSortBy::NameDesc => Self::NameDesc,
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/prefs/settings/sort.rs"]
mod tests;

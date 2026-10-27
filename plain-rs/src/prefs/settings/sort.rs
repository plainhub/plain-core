#[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileSortBy {
    DateAsc,
    DateDesc,
    SizeAsc,
    SizeDesc,
    NameAsc,
    NameDesc,
    TakenAtDesc,
}

impl<'de> serde::Deserialize<'de> for FileSortBy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        super::enum_value::deserialize(
            deserializer,
            &[
                (0, "DATE_ASC", Self::DateAsc),
                (1, "DATE_DESC", Self::DateDesc),
                (2, "SIZE_ASC", Self::SizeAsc),
                (3, "SIZE_DESC", Self::SizeDesc),
                (4, "NAME_ASC", Self::NameAsc),
                (5, "NAME_DESC", Self::NameDesc),
                (6, "TAKEN_AT_DESC", Self::TakenAtDesc),
            ],
        )
    }
}

impl FileSortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            FileSortBy::DateAsc => "DATE_ASC",
            FileSortBy::DateDesc => "DATE_DESC",
            FileSortBy::SizeAsc => "SIZE_ASC",
            FileSortBy::SizeDesc => "SIZE_DESC",
            FileSortBy::NameAsc => "NAME_ASC",
            FileSortBy::NameDesc => "NAME_DESC",
            FileSortBy::TakenAtDesc => "TAKEN_AT_DESC",
        }
    }
}

#[cfg(feature = "filesystem")]
impl From<FileSortBy> for crate::files::filesystem::SortBy {
    fn from(sort_by: FileSortBy) -> Self {
        use crate::files::filesystem::SortBy;
        match sort_by {
            FileSortBy::DateAsc => SortBy::DateAsc,
            FileSortBy::DateDesc => SortBy::DateDesc,
            FileSortBy::SizeAsc => SortBy::SizeAsc,
            FileSortBy::SizeDesc => SortBy::SizeDesc,
            FileSortBy::NameAsc | FileSortBy::TakenAtDesc => SortBy::NameAsc,
            FileSortBy::NameDesc => SortBy::NameDesc,
        }
    }
}

use plain_libs::rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileKind {
    File,
    Directory,
    Image,
    Video,
    Audio,
    Doc,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Directory => "DIRECTORY",
            Self::Image => "IMAGE",
            Self::Video => "VIDEO",
            Self::Audio => "AUDIO",
            Self::Doc => "DOC",
        }
    }

    pub fn data_type(self) -> crate::features::enums::DataType {
        use crate::features::enums::DataType;
        match self {
            Self::Audio => DataType::Audio,
            Self::Video => DataType::Video,
            Self::Image => DataType::Image,
            Self::Doc => DataType::Doc,
            Self::File | Self::Directory => DataType::File,
        }
    }
}

impl TryFrom<crate::features::enums::DataType> for FileKind {
    type Error = crate::infrastructure::error::DomainError;
    fn try_from(value: crate::features::enums::DataType) -> Result<Self, Self::Error> {
        use crate::features::enums::DataType;
        match value {
            DataType::Audio => Ok(Self::Audio),
            DataType::Video => Ok(Self::Video),
            DataType::Image => Ok(Self::Image),
            DataType::Doc => Ok(Self::Doc),
            DataType::File => Ok(Self::File),
            _ => Err(crate::infrastructure::error::DomainError::Other(
                "Unsupported file catalog type".into(),
            )),
        }
    }
}

impl plain_libs::rusqlite::ToSql for FileKind {
    fn to_sql(&self) -> plain_libs::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_str().as_bytes(),
        )))
    }
}
impl FromSql for FileKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "FILE" => Ok(Self::File),
            "DIRECTORY" => Ok(Self::Directory),
            "IMAGE" => Ok(Self::Image),
            "VIDEO" => Ok(Self::Video),
            "AUDIO" => Ok(Self::Audio),
            "DOC" => Ok(Self::Doc),
            _ => Err(plain_libs::rusqlite::types::FromSqlError::Other(
                "Invalid file kind".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum IndexMode {
    #[default]
    Unconfigured,
    Disabled,
    Allowlist,
    Blocklist,
}

impl IndexMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unconfigured => "UNCONFIGURED",
            Self::Disabled => "DISABLED",
            Self::Allowlist => "ALLOWLIST",
            Self::Blocklist => "BLOCKLIST",
        }
    }
}
impl plain_libs::rusqlite::ToSql for IndexMode {
    fn to_sql(&self) -> plain_libs::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_str().as_bytes(),
        )))
    }
}
impl FromSql for IndexMode {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "UNCONFIGURED" => Ok(Self::Unconfigured),
            "DISABLED" => Ok(Self::Disabled),
            "ALLOWLIST" => Ok(Self::Allowlist),
            "BLOCKLIST" => Ok(Self::Blocklist),
            _ => Err(plain_libs::rusqlite::types::FromSqlError::Other(
                "Invalid index mode".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct VolumeRow {
    pub id: String,
    pub identity: String,
    pub path: String,
    pub is_available: bool,
    pub index_mode: IndexMode,
    pub included_dirs: Vec<String>,
    pub excluded_dirs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FileRow {
    pub id: String,
    pub volume_id: String,
    pub path: String,
    pub file_name: String,
    pub kind: FileKind,
    pub file_identity: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: String,
    pub missing: bool,
    pub favorited: bool,
    pub trashed_at: Option<String>,
    pub original_path: Option<String>,
    pub duration_ms: Option<i64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub rotation: Option<i32>,
    pub taken_at: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub album_id: Option<String>,
    pub cover_image_id: Option<String>,
    pub indexed: bool,
}

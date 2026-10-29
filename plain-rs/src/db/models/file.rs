use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};

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

    pub fn data_type(self) -> crate::enums::DataType {
        use crate::enums::DataType;
        match self {
            Self::Audio => DataType::Audio,
            Self::Video => DataType::Video,
            Self::Image => DataType::Image,
            Self::Doc => DataType::Doc,
            Self::File | Self::Directory => DataType::File,
        }
    }
}

impl TryFrom<crate::enums::DataType> for FileKind {
    type Error = crate::error::DomainError;
    fn try_from(value: crate::enums::DataType) -> Result<Self, Self::Error> {
        use crate::enums::DataType;
        match value {
            DataType::Audio => Ok(Self::Audio),
            DataType::Video => Ok(Self::Video),
            DataType::Image => Ok(Self::Image),
            DataType::Doc => Ok(Self::Doc),
            DataType::File => Ok(Self::File),
            _ => Err(crate::error::DomainError::Other(
                "Unsupported file catalog type".into(),
            )),
        }
    }
}

impl rusqlite::ToSql for FileKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
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
            _ => Err(rusqlite::types::FromSqlError::Other(
                "Invalid file kind".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct VolumeRow {
    pub id: i64,
    pub identity: String,
    pub path: String,
    pub is_available: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FileRow {
    pub id: i64,
    pub volume_id: i64,
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
}

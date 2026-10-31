use plain_rs::{db::MediaFileMetadataRead, enums::DataType};
use std::{fs::File, io::BufReader, path::Path};

pub struct FileReads {
    pub media: Vec<MediaFileMetadataRead>,
    pub generic: Vec<String>,
}

pub fn read_files(
    directory: &Path,
    cache: &Path,
    excluded_roots: &[String],
    paths: Vec<String>,
) -> Result<FileReads, String> {
    for path in &paths {
        plain_rs::services::files::platform::authorize_native(Path::new(path), false)
            .map_err(|error| error.to_string())?;
    }
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut files = FileReads {
        media: Vec::new(),
        generic: Vec::new(),
    };
    for path in paths {
        let path = Path::new(&path)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !path.is_dir() && !path.is_file() {
            return Err("Regular completed file required".into());
        }
        let name = path
            .to_str()
            .ok_or("Invalid completed file path encoding")?;
        if plain_rs::media::catalog_paths::is_file_excluded_at(
            name,
            &directory,
            cache,
            excluded_roots,
        ) {
            continue;
        }
        if path.is_dir()
            || plain_rs::media::catalog_paths::is_media_excluded_at(
                name,
                &directory,
                cache,
                excluded_roots,
            )
        {
            files.generic.push(name.to_owned());
            continue;
        }
        let kind = plain_rs::media::catalog_paths::infer_type(name);
        let Some(data_type) = [
            DataType::Audio,
            DataType::Video,
            DataType::Image,
            DataType::Doc,
        ]
        .into_iter()
        .find(|data_type| data_type.media_type_str() == Some(kind)) else {
            files.generic.push(name.to_owned());
            continue;
        };
        let read = plain_rs::media::file_metadata::read(&path, data_type, |reader: File| {
            image::ImageReader::new(BufReader::new(reader))
                .with_guessed_format()
                .ok()?
                .into_dimensions()
                .ok()
        })
        .map_err(|error| error.to_string())?;
        files.media.push(read);
    }
    Ok(files)
}

pub fn record_generic(db: &plain_rs::db::Db, paths: &[String]) -> Result<Vec<i32>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let encoded = serde_json::to_string(paths).map_err(|error| error.to_string())?;
    let mut types = db.with_conn(|conn| {
        conn.prepare("SELECT DISTINCT data_type FROM file_catalog WHERE missing=0 AND path IN (SELECT value FROM json_each(?1))")?
            .query_map([encoded], |row| row.get::<_,i32>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()
    }).map_err(|error| error.to_string())?;
    let rows = db
        .record_file_paths(paths, Some(plain_rs::db::FileKind::File))
        .map_err(|error| error.to_string())?;
    types.extend(rows.into_iter().map(|row| row.kind.data_type().kind()));
    Ok(types)
}

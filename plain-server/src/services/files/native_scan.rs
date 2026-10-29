use super::media_scans::MediaScans;
use plain_rs::db::Db;
use std::path::Path;

pub(crate) fn completed(
    db: &Db,
    scans: &MediaScans,
    directory: &Path,
    paths: Vec<String>,
) -> Result<(), String> {
    let policy = scans.policy()?;
    let files = super::media_index::read_files(
        directory,
        &policy.cache_directory,
        &policy.excluded_roots,
        paths,
    )?;
    let (rows, generic_types) = scans.record_completion(|| {
        let generic_types = super::media_index::record_generic(db, &files.generic)?;
        let rows = db
            .record_media_file_reads(&files.media)
            .map_err(|error| error.to_string())?;
        Ok((rows, generic_types))
    })?;
    scans.publish_catalog_types(rows.iter().map(|row| row.data_type).chain(generic_types));
    Ok(())
}

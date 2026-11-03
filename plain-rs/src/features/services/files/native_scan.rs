use crate::features::services::files::media_scans::MediaScans;
use crate::infrastructure::db::Db;
use std::path::Path;

pub async fn completed(
    db: &Db,
    scans: &std::sync::Arc<MediaScans>,
    directory: &Path,
    paths: Vec<String>,
) -> Result<Vec<crate::infrastructure::db::MediaFileRow>, String> {
    let db = db.clone();
    let recorder = scans.clone();
    let directory = directory.to_owned();
    let (rows, generic_types) =
        tokio::task::spawn_blocking(move || record(&db, &recorder, &directory, paths))
            .await
            .map_err(|error| error.to_string())??;
    scans.publish_catalog_types(rows.iter().map(|row| row.data_type).chain(generic_types));
    Ok(rows)
}

fn record(
    db: &Db,
    scans: &MediaScans,
    directory: &Path,
    paths: Vec<String>,
) -> Result<(Vec<crate::infrastructure::db::MediaFileRow>, Vec<i32>), String> {
    if paths.iter().any(|path| !Path::new(path).is_absolute()) {
        return Err("Absolute completed file path required".into());
    }
    let policy = scans.policy()?;
    let eligible: Vec<_> = paths
        .iter()
        .filter(|path| {
            policy
                .scope
                .as_ref()
                .is_none_or(|scope| scope.contains(Path::new(path)))
        })
        .cloned()
        .collect();
    let files = crate::features::services::files::media_index::read_files(
        directory,
        &policy.cache_directory,
        &policy.excluded_roots,
        eligible,
        policy.scope.as_ref(),
    )?;
    let recorded = scans.record_with_scope(|scope| {
        if let Some(scope) = scope {
            db.update_index_visibility(scope, Some(&paths))
                .map_err(|error| error.to_string())?;
        }
        let generic = files
            .generic
            .into_iter()
            .filter(|path| scope.is_none_or(|scope| scope.contains(Path::new(path))))
            .collect::<Vec<_>>();
        let media = files
            .media
            .into_iter()
            .filter(|read| scope.is_none_or(|scope| scope.contains(Path::new(&read.metadata.path))))
            .collect::<Vec<_>>();
        let generic_types =
            crate::features::services::files::media_index::record_generic(db, &generic)?;
        let rows = db
            .record_media_file_reads(&media)
            .map_err(|error| error.to_string())?;
        Ok((rows, generic_types))
    })?;
    Ok(recorded)
}

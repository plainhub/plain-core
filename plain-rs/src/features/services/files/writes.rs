use crate::features::services::files::platform::FilePlatform;
use plain_libs::filesystem::writes::Operation;

pub async fn execute(
    platform: &FilePlatform,
    operation: Operation,
) -> Result<plain_libs::filesystem::record::FileRecord, String> {
    let path = operation.path().to_owned();
    if !std::path::Path::new(&path).is_absolute() {
        return Err("absolute file path required".into());
    }
    platform
        .authorize(&path, true)
        .await
        .map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || operation.apply())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    platform
        .scan(std::slice::from_ref(&path))
        .await
        .map_err(|error| error.to_string())?;
    let entry = plain_libs::filesystem::stat(std::path::Path::new(&path))
        .await
        .map_err(|e| e.to_string())?;
    let mut record = plain_libs::filesystem::record::FileRecord::from(entry);
    record.permission = "rw".into();
    Ok(record)
}

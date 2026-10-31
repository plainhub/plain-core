use plain_rs::files::filesystem::writes::Operation;
use plain_rs::services::files::platform::FilePlatform;

pub async fn execute(
    platform: &FilePlatform,
    operation: Operation,
) -> Result<plain_rs::files::filesystem::record::FileRecord, String> {
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
    let entry = plain_rs::files::filesystem::stat(std::path::Path::new(&path))
        .await
        .map_err(|e| e.to_string())?;
    let mut record = plain_rs::files::filesystem::record::FileRecord::from(entry);
    record.permission = "rw".into();
    Ok(record)
}

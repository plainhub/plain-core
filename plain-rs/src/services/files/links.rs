use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::Value;

pub struct File {
    pub path: String,
    pub media_id: String,
    pub name: String,
}

pub fn resolve(state: &CoreRuntime, id: &str, share_id: Option<&str>) -> anyhow::Result<File> {
    if let Some(share_id) = share_id {
        let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let path = shares
            .resolve_file(share_id, id)?
            .ok_or_else(|| anyhow::anyhow!("Shared file unavailable"))?;
        let name = path.rsplit('/').next().unwrap_or_default().to_owned();
        return Ok(File {
            path,
            media_id: String::new(),
            name,
        });
    }
    let plain = plain_rs::crypto::xchacha_decrypt(
        &plain_rs::prefs::ensure_url_token(&state.prefs),
        &plain_rs::utils::base64::base64_decode(id),
    )
    .ok_or_else(|| anyhow::anyhow!("Invalid file id"))?;
    let plain = String::from_utf8(plain)?;
    let (path, media_id, name) = if plain.starts_with('{') {
        let value: Value = serde_json::from_str(&plain)?;
        (
            value["path"].as_str().unwrap_or_default().to_owned(),
            value["mediaId"].as_str().unwrap_or_default().to_owned(),
            value["name"].as_str().unwrap_or_default().to_owned(),
        )
    } else {
        (plain, String::new(), String::new())
    };
    Ok(File {
        path: plain_rs::services::files::references::resolve(
            state.db.clone(),
            state.directory.clone(),
            &path,
        )?,
        media_id,
        name,
    })
}

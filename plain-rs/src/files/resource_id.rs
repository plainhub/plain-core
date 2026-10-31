#[derive(Debug)]
pub enum FileIdError {
    InvalidId,
    Forbidden,
}

pub fn path_from_file_id(id: &str, prefs: &plain_rs::prefs::Prefs) -> Result<String, FileIdError> {
    use plain_rs::files::resource_link::{self, ResourceType, Scope};
    let (scope, payload) = resource_link::parse(id).map_err(|_| FileIdError::InvalidId)?;
    if scope != Scope::Global {
        return Err(FileIdError::Forbidden);
    }
    let token = prefs
        .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
        .map_err(|_| FileIdError::Forbidden)?
        .ok_or(FileIdError::Forbidden)?;
    let resource = resource_link::open(
        &scope,
        &plain_rs::utils::base64::base64_decode(&token),
        payload,
    )
    .map_err(|_| FileIdError::Forbidden)?;
    match resource.r#type {
        ResourceType::File => Ok(resource.id),
        ResourceType::AppFile => Ok(format!("fid:{}", resource.id)),
        _ => Err(FileIdError::Forbidden),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/files/resource_id.rs"]
mod tests;

use super::{Prefs, Result};
use crate::prefs::pref_keys::MEDIA_SOURCE_DIRS;

pub fn source_directories(prefs: &Prefs) -> Result<Vec<String>> {
    Ok(prefs.get(MEDIA_SOURCE_DIRS)?.unwrap_or_default())
}

pub fn set_source_directories(prefs: &Prefs, directories: &[String]) -> Result<()> {
    prefs.set(MEDIA_SOURCE_DIRS, directories)?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/prefs/media.rs"]
mod tests;

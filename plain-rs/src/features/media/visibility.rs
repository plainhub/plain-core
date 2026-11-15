use crate::features::prefs::{Prefs, pref_keys::MEDIA_EXCLUDED_DIRS};
use std::path::Path;

pub fn validate(value: &plain_libs::serde_json::Value) -> plain_libs::anyhow::Result<()> {
    let roots: Vec<String> = plain_libs::serde_json::from_value(value.clone())?;
    plain_libs::anyhow::ensure!(
        roots.iter().all(|root| Path::new(root).is_absolute()),
        "Absolute media excluded directory required"
    );
    Ok(())
}

pub fn selection(prefs: &Prefs) -> plain_libs::anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut clauses = Vec::new();
    let mut args = Vec::new();
    for root in prefs
        .get::<Vec<String>>(MEDIA_EXCLUDED_DIRS)?
        .unwrap_or_default()
    {
        let root = crate::features::files::index_scope::normalized(Path::new(&root));
        let prefix = format!("{}/", root.trim_end_matches('/'));
        clauses.push("NOT (_data = ? OR substr(_data,1,length(?)) = ?)".into());
        args.extend([root, prefix.clone(), prefix]);
    }
    Ok((clauses, args))
}

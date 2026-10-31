use anyhow::{Result, ensure};
pub fn download_url(raw: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw)?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && matches!(url.path(), "/fs" | "/zip/dir"),
        "Invalid shared download URL"
    );
    let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
    let id = query
        .get("id")
        .ok_or_else(|| anyhow::anyhow!("Missing shared file ID"))?;
    let (scope, _) = crate::files::resource_link::parse(id)?;
    ensure!(
        matches!(scope, crate::files::resource_link::Scope::Share(_)),
        "Invalid shared file scope"
    );
    Ok(url)
}

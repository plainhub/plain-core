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
    ensure!(
        query.get("sid").is_some_and(|id| !id.is_empty())
            && query.get("id").is_some_and(|id| !id.is_empty()),
        "Missing shared file ID"
    );
    Ok(url)
}

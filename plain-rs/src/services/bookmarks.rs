use anyhow::{Result, ensure};

#[derive(async_graphql::SimpleObject)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub favicon_url: Option<String>,
}

pub async fn fetch_page_metadata(url: &str) -> Result<PageMetadata> {
    let url = reqwest::Url::parse(url)?;
    ensure!(
        ["http", "https"].contains(&url.scheme()),
        "unsupported page URL"
    );
    let html = plain_rs::services::feeds::fetch_text(url.as_str(), 2 * 1024 * 1024).await?;
    Ok(parse_page_metadata(&html, url.as_str()))
}

fn parse_page_metadata(html: &str, base: &str) -> PageMetadata {
    let dom = plain_rs::utils::html_to_markdown::dom::Dom::parse(html);
    let title = dom
        .0
        .iter()
        .find(|node| node.tag == "meta" && node.attr("property") == "og:title")
        .map(|node| node.attr("content").to_owned())
        .or_else(|| {
            dom.0
                .iter()
                .enumerate()
                .find(|(_, node)| node.tag == "title")
                .map(|(id, _)| dom.text(id))
        })
        .map(|t| t.trim().chars().take(200).collect::<String>())
        .filter(|t| !t.is_empty());
    let icon = dom
        .0
        .iter()
        .filter(|node| node.tag == "link" && node.attr("rel").to_lowercase().contains("icon"))
        .find_map(|node| plain_rs::services::feeds::assets::absolute_url(base, node.attr("href")))
        .or_else(|| plain_rs::services::feeds::assets::absolute_url(base, "/favicon.ico"));
    PageMetadata {
        title,
        favicon_url: icon,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/services/bookmarks.rs"]
mod tests;

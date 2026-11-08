use plain_libs::protocols::http_client;
use crate::infrastructure::db::{Db, bookmark};
use anyhow::{Result, ensure};
use std::collections::{HashMap, HashSet};

pub fn parse_bookmark_url(url: &str) -> Result<http_client::Url> {
    let url = http_client::Url::parse(url)?;
    ensure!(
        ["http", "https"].contains(&url.scheme()),
        "unsupported page URL"
    );
    Ok(url)
}

pub fn add_bookmarks(
    db: &Db,
    urls: Vec<String>,
    group_id: &str,
) -> Result<Vec<bookmark::DBookmark>> {
    let urls = urls
        .into_iter()
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();
    for url in &urls {
        parse_bookmark_url(url)?;
    }
    urls.into_iter()
        .map(|url| {
            let row = bookmark::DBookmark::new(&url, group_id);
            bookmark::insert_bookmark(db, &row)?;
            Ok(row)
        })
        .collect()
}

pub fn import_bookmarks(db: &Db, html: &str) -> Result<i32> {
    let dom = plain_libs::utils::html_to_markdown::dom::Dom::parse(html);
    let mut entries = Vec::new();
    for (id, node) in dom.0.iter().enumerate().filter(|(_, node)| node.tag == "a") {
        let url = node.attr("href").trim();
        if parse_bookmark_url(url).is_err() {
            continue;
        }
        let mut parent = node.parent;
        while parent.is_some_and(|parent| dom.0[parent].tag != "dl") {
            parent = dom.0[parent.unwrap()].parent;
        }
        let Some(list) = parent else { continue };
        let group = dom.0[list]
            .parent
            .and_then(|parent| {
                dom.0[parent]
                    .children
                    .iter()
                    .find(|&&child| dom.0[child].tag == "h3")
            })
            .map(|&heading| dom.text(heading).trim().to_owned())
            .unwrap_or_default();
        let title = dom.text(id).trim().to_owned();
        entries.push((url.to_owned(), title, group));
    }
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let mut urls = {
            let mut stmt = tx.prepare("SELECT url FROM bookmarks")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<HashSet<_>>>()?;
            rows
        };
        let mut groups = HashMap::new();
        {
            let mut stmt = tx.prepare("SELECT name,id FROM bookmark_groups ORDER BY id")?;
            for row in stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (name, id) = row?;
                groups.entry(name).or_insert(id);
            }
        }
        let mut count = 0;
        for (url, title, group) in entries {
            if !urls.insert(url.clone()) {
                continue;
            }
            let group_id = if group.is_empty() {
                String::new()
            } else if let Some(id) = groups.get(&group) {
                id.clone()
            } else {
                let created = bookmark::DBookmarkGroup::new(&group);
                bookmark::insert_bookmark_group_with_conn(&tx, &created)?;
                groups.insert(group, created.id.clone());
                created.id
            };
            let mut row = bookmark::DBookmark::new(&url, &group_id);
            if !title.is_empty() {
                row.title = title;
            }
            bookmark::insert_bookmark_with_conn(&tx, &row)?;
            count += 1;
        }
        tx.commit()?;
        Ok::<_, rusqlite::Error>(count)
    })
    .map_err(Into::into)
}

#[derive(plain_graphql::SimpleObject)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub favicon_url: Option<String>,
}

pub async fn fetch_page_metadata(url: &str) -> Result<PageMetadata> {
    let url = parse_bookmark_url(url)?;
    let html = crate::features::services::feeds::fetch_text(url.as_str(), 2 * 1024 * 1024).await?;
    Ok(parse_page_metadata(&html, url.as_str()))
}

fn parse_page_metadata(html: &str, base: &str) -> PageMetadata {
    let dom = plain_libs::utils::html_to_markdown::dom::Dom::parse(html);
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
        .find_map(|node| {
            crate::features::services::feeds::assets::absolute_url(base, node.attr("href"))
        })
        .or_else(|| crate::features::services::feeds::assets::absolute_url(base, "/favicon.ico"));
    PageMetadata {
        title,
        favicon_url: icon,
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/bookmarks.rs"]
mod tests;

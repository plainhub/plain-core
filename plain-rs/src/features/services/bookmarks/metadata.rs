use super::fetch_page_metadata;
use crate::infrastructure::db::{bookmark, bookmark::DBookmark};
use crate::infrastructure::events::{WS_BOOKMARK_UPDATED, WsEvent};
use crate::runtime::Runtime;
use plain_libs::serde_json::json;
use tokio::sync::broadcast;

pub fn start_metadata_update(state: Runtime, items: Vec<DBookmark>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        if *stop.borrow() {
            return;
        }
        for item in items {
            tokio::select! {
                biased;
                _ = stop.changed() => return,
                result = update(&state, &item) => {
                    if let Err(error) = result {
                        log::warn!("Bookmark metadata update failed: {error}");
                    }
                }
            }
        }
    })
}

async fn update(state: &Runtime, item: &DBookmark) -> plain_libs::anyhow::Result<()> {
    let metadata = fetch_page_metadata(&item.url).await?;
    let favicon = match metadata.favicon_url {
        Some(url) => match state.feed_assets.cache_bookmark_icon(&url).await {
            Ok(path) => Some(path),
            Err(error) => {
                log::warn!("Bookmark favicon fetch failed: {error}");
                None
            }
        },
        None => None,
    };
    let saved = bookmark::update_metadata(
        &state.db,
        item,
        metadata.title.as_deref(),
        favicon.as_deref(),
    );
    if let Some(path) = &favicon {
        if !matches!(&saved, Ok(Some((row, previous))) if previous == &item.favicon_path && row.favicon_path == *path)
        {
            state.feed_assets.release(path);
        }
    }
    if let Some((row, _)) = saved? {
        emit_updated(&state.events, &[row]);
    }
    Ok(())
}

pub fn emit_updated(events: &broadcast::Sender<WsEvent>, items: &[DBookmark]) {
    if items.is_empty() {
        return;
    }
    let payload = items
        .iter()
        .map(|b| {
            json!({
                "id": b.id,
                "url": b.url,
                "title": b.title,
                "faviconPath": b.favicon_path,
                "groupId": b.group_id.as_deref().unwrap_or_default(),
                "pinned": b.pinned,
                "clickCount": b.click_count,
                "lastClickedAt": b.last_clicked_at,
                "sortOrder": b.sort_order,
                "createdAt": b.created_at,
                "updatedAt": b.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let _ = events.send(WsEvent::broadcast(
        WS_BOOKMARK_UPDATED,
        json!(payload).to_string(),
    ));
}

use super::*;
use crate::features::capability::Capability;
use crate::features::enums::DataType;
use crate::features::files::trash::{TrashItem, is_trashed_path};
use crate::features::media::actions::Action;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: String,
    path: PathBuf,
    display_name: String,
    volume_root: PathBuf,
    media_type: i32,
    mime_type: Option<String>,
    size: i64,
}

pub(super) const PREFIX: &str = "mediastore:";

impl FileTasks {
    pub(super) async fn has_provider_trash(&self) -> Result<bool> {
        let host = &self.hooks.platform.host;
        Ok(host.uses_system_media_catalog()
            && crate::features::services::capabilities::snapshot(host)
                .await?
                .contains(&Capability::MediaTrash))
    }

    pub(super) async fn provider_trash(&self) -> Result<Vec<TrashItem>> {
        if !self.has_provider_trash().await? {
            return Ok(Vec::new());
        }
        let rows: Vec<Row> = plain_libs::serde_json::from_value(
            self.hooks
                .platform
                .host
                .call("mediaProviderTrashRows", plain_libs::serde_json::json!({}))
                .await
                .map_err(plain_libs::anyhow::Error::msg)?,
        )?;
        let mut items = Vec::new();
        for row in rows {
            let Some(kind) = crate::features::services::files::platform::media_rows::kind(
                row.media_type,
                row.mime_type.as_deref(),
            ) else {
                continue;
            };
            let path = row
                .path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid media path"))?;
            if is_trashed_path(path) {
                continue;
            }
            if !row.volume_root.is_absolute()
                || row.id.parse::<u64>().is_err()
                || Path::new(&row.display_name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    != Some(row.display_name.as_str())
            {
                bail!("Invalid provider trash entry");
            }
            let relative = row
                .path
                .strip_prefix(&row.volume_root)?
                .to_str()
                .ok_or_else(|| anyhow!("Invalid volume path"))?;
            items.push(TrashItem {
                id: format!("{PREFIX}{}:{}:{path}", kind.kind(), row.id),
                kind: "file".into(),
                original_path: row
                    .path
                    .with_file_name(&row.display_name)
                    .to_string_lossy()
                    .into_owned(),
                disk: row.volume_root.to_string_lossy().into_owned(),
                trash_rel_path: relative.to_owned(),
                path: row.path,
                size: Some(row.size),
                entry_count: None,
                deleted_at: None,
                uid: 0,
                gid: 0,
                mode: 0,
            });
        }
        Ok(items)
    }

    pub(super) async fn selected_provider_trash(
        &self,
        references: &[String],
    ) -> Result<Vec<TrashItem>> {
        if references
            .iter()
            .any(|reference| reference.starts_with(PREFIX))
        {
            self.provider_trash().await
        } else {
            Ok(Vec::new())
        }
    }

    pub(super) async fn apply_provider_reference(
        &self,
        reference: &str,
        action: Action,
        current: &[TrashItem],
    ) -> Result<Option<bool>> {
        let Some(parts) = reference.strip_prefix(PREFIX) else {
            return Ok(None);
        };
        let mut parts = parts.splitn(3, ':');
        let kind = parts
            .next()
            .and_then(|kind| kind.parse().ok())
            .and_then(DataType::from_kind)
            .filter(|kind| kind.media_type_str().is_some())
            .ok_or_else(|| anyhow!("Invalid media trash type"))?;
        let id = parts
            .next()
            .ok_or_else(|| anyhow!("Missing media identity"))?;
        let path = parts.next().ok_or_else(|| anyhow!("Missing media path"))?;
        if !current.iter().any(|row| row.id == reference) {
            return Ok(Some(false));
        }
        self.hooks.platform.authorize(path, true).await?;
        self.media_actions
            .apply_file(kind, action, id.into(), path.into())
            .await?;
        Ok(Some(true))
    }
}

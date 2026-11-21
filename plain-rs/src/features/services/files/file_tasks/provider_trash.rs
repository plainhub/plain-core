use super::*;
use crate::features::capability::Capability;
use crate::features::enums::DataType;
use crate::features::files::trash::{TrashItem, is_trashed_path};
use crate::features::media::actions::Action;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: String,
    path: PathBuf,
    display_name: String,
    volume_root: PathBuf,
    media_type: i32,
    mime_type: Option<String>,
}


impl FileTasks {
    pub(super) async fn has_provider_trash(&self) -> Result<bool> {
        let host = &self.hooks.platform.host;
        Ok(host.uses_system_media_catalog()
            && crate::features::services::capabilities::snapshot(host)
                .await?
                .contains(&Capability::MediaTrash))
    }

    pub(super) async fn provider_trash(&self) -> Result<Vec<(TrashItem, DataType)>> {
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
            row.path.strip_prefix(&row.volume_root)?;
            items.push((TrashItem {
                id: row.id,
                original_path: row
                    .path
                    .with_file_name(&row.display_name)
                    .to_string_lossy()
                    .into_owned(),
                disk: row.volume_root.to_string_lossy().into_owned(),
                path: row.path,
            }, kind));
        }
        Ok(items)
    }

    pub(super) async fn selected_provider_trash(
        &self,
        paths: &[String],
    ) -> Result<HashMap<PathBuf, (String, DataType)>> {
        for path in paths {
            if !Path::new(path).is_absolute() {
                bail!("Absolute trash path required");
            }
        }
        if paths.iter().all(|path| is_trashed_path(path)) {
            return Ok(HashMap::new());
        }
        Ok(self.provider_trash().await?.into_iter()
            .map(|(row, kind)| (row.path, (row.id, kind))).collect())
    }

    pub(super) async fn apply_provider_path(
        &self,
        path: &str,
        action: Action,
        current: &mut HashMap<PathBuf, (String, DataType)>,
    ) -> Result<Option<bool>> {
        if is_trashed_path(path) {
            return Ok(None);
        }
        let Some((id, kind)) = current.remove(Path::new(path)) else {
            return Ok(Some(false));
        };
        self.hooks.platform.authorize(path, true).await?;
        self.media_actions.apply_file(kind, action, id, path.into()).await?;
        Ok(Some(true))
    }
}

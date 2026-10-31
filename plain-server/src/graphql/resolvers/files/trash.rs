use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use async_graphql::{Context, Enum, ID, Object, Result, SimpleObject};
use plain_rs::files::trash::{SortOrder, Trash, TrashItem};
use plain_rs::services::files::file_tasks::FileTasks;
use plain_rs::types::{Instant, Long};
use std::sync::Arc;

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum TrashedFileSortBy {
    DateAsc,
    DateDesc,
    SizeAsc,
    SizeDesc,
    NameAsc,
    NameDesc,
}
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum TrashedFileType {
    File,
    Dir,
}
#[derive(SimpleObject)]
struct TrashedFile {
    id: ID,
    r#type: TrashedFileType,
    original_path: String,
    disk: String,
    trash_rel_path: String,
    deleted_at: Instant,
    uid: i32,
    gid: i32,
    mode: i32,
    size_bytes: Option<Long>,
    entry_count: Option<i32>,
    display_name: String,
    trashed_path: String,
}
impl TryFrom<TrashItem> for TrashedFile {
    type Error = async_graphql::Error;
    fn try_from(row: TrashItem) -> Result<Self> {
        let path = Trash::physical_path(&row).to_string_lossy().into_owned();
        Ok(Self {
            id: row.id.into(),
            r#type: match row.kind.as_str() {
                "file" => TrashedFileType::File,
                "dir" => TrashedFileType::Dir,
                _ => return Err("invalid trash kind".into()),
            },
            display_name: std::path::Path::new(&row.original_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            original_path: row.original_path,
            disk: row.disk,
            trash_rel_path: row.trash_rel_path,
            deleted_at: Instant(
                chrono::DateTime::from_timestamp(row.deleted_at, 0)
                    .ok_or("invalid trash timestamp")?,
            ),
            uid: row.uid.try_into()?,
            gid: row.gid.try_into()?,
            mode: row.mode.try_into()?,
            size_bytes: row.size.map(Long),
            entry_count: row.entry_count.map(i32::try_from).transpose()?,
            trashed_path: path,
        })
    }
}
fn tasks<'a>(ctx: &'a Context<'_>) -> Result<&'a Arc<FileTasks>> {
    public_gate::require(
        ctx,
        ctx.data::<Arc<plain_rs::prefs::Prefs>>()?,
        &["WRITE_EXTERNAL_STORAGE"],
    )?;
    ctx.data::<Arc<FileTasks>>()
}
#[derive(Default)]
pub struct TrashQuery;
#[Object]
impl TrashQuery {
    async fn trashed_files(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: TrashedFileSortBy,
    ) -> Result<Vec<TrashedFile>> {
        let order = match sort_by {
            TrashedFileSortBy::DateAsc => SortOrder::DeletedAtOldest,
            TrashedFileSortBy::DateDesc => SortOrder::DeletedAtNewest,
            TrashedFileSortBy::SizeAsc => SortOrder::SizeAsc,
            TrashedFileSortBy::SizeDesc => SortOrder::SizeDesc,
            TrashedFileSortBy::NameAsc => SortOrder::NameAsc,
            TrashedFileSortBy::NameDesc => SortOrder::NameDesc,
        };
        tasks(ctx)?
            .trashed(offset.try_into()?, limit.try_into()?, query, order)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
    async fn trashed_file_count(&self, ctx: &Context<'_>) -> Result<i32> {
        Ok(tasks(ctx)?.trash_count().await?.try_into()?)
    }
}
#[derive(Default)]
pub struct TrashMutation;
#[Object]
impl TrashMutation {
    async fn trash_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.trash_paths(paths).await?.try_into()?,
        })
    }
    async fn restore_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.restore_paths(paths).await?.try_into()?,
        })
    }
    async fn delete_trashed_file(&self, ctx: &Context<'_>, path: String) -> Result<bool> {
        Ok(tasks(ctx)?.delete_trashed(path).await?)
    }
}

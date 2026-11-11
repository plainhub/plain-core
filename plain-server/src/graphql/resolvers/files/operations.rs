//! Public `/graphql` roots that write: file operations, media-library
//! actions and the resumable-upload merge state.
//!
use plain_rs::features::access::AccessFeature;
use crate::graphql::resolvers::files::types::File;
use crate::graphql::resolvers::media::public::MediaDataType;
use crate::graphql::types::ActionResult;
use crate::graphql::types::facts::text;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Enum, Object, SimpleObject};

use plain_rs::features::services::files::provider_query as provider_plan;
use plain_rs::features::services::files::provider_query::Provider;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Long;
use serde_json::{Value, json};
use std::sync::Arc;

const STORAGE: AccessFeature = AccessFeature::Files;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum MergeTaskStatus {
    #[default]
    None,
    Started,
    Merging,
    Done,
    Failed,
}

impl MergeTaskStatus {
    fn parse(value: &str) -> Self {
        match value {
            "STARTED" => Self::Started,
            "MERGING" => Self::Merging,
            "DONE" => Self::Done,
            "FAILED" => Self::Failed,
            _ => Self::None,
        }
    }
}

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct MergeTask {
    pub status: MergeTaskStatus,
    pub value: Option<String>,
    #[graphql(name = "mergedSize")]
    pub merged_size: Option<Long>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct UploadQuery;

#[Object]
impl UploadQuery {
    /// Chunk state for a client-chosen chunk-set id, as `index:byteSize`
    /// strings. An empty list means the upload never started or was
    /// discarded — the caller cannot tell, and does not need to.
    async fn uploaded_chunks(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> plain_graphql::Result<Vec<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        ctx.data::<Arc<plain_rs::features::services::files::upload_chunks::UploadChunks>>()?
            .list(&file_id)
            .await
            .map_err(plain_graphql::Error::new)
    }

    /// `NONE` when no merge was ever started for this id, which is what a
    /// client sees before it calls `mergeChunks`.
    fn merge_status(&self, ctx: &Context<'_>, file_id: String) -> plain_graphql::Result<MergeTask> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(merge_task(
            &ctx.data_unchecked::<std::sync::Arc<plain_rs::features::files::uploads::Runtime>>()
                .status(&file_id),
        ))
    }
}

#[derive(Default)]
pub struct FileOpsMutation;

#[Object]
impl FileOpsMutation {
    async fn create_file(&self, ctx: &Context<'_>, path: String) -> plain_graphql::Result<File> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let record = plain_rs::features::services::files::writes::execute(
            ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?,
            plain_libs::filesystem::writes::Operation::CreateFile { path },
        )
        .await
        .map_err(plain_graphql::Error::new)?;
        Ok(written_file(record))
    }
    async fn delete_files(
        &self,
        ctx: &Context<'_>,
        paths: Vec<String>,
    ) -> plain_graphql::Result<ActionResult> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let tasks =
            ctx.data::<Arc<plain_rs::features::services::files::file_tasks::FileTasks>>()?;
        Ok(ActionResult {
            affected_count: tasks.delete_many(paths).await?,
        })
    }

    async fn create_zip_archive(
        &self,
        ctx: &Context<'_>,
        paths: Vec<String>,
        target: String,
    ) -> plain_graphql::Result<String> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let state = ctx.data::<crate::server::ServerState>()?;
        plain_rs::features::services::files::archive_operations::pack(
            &state.file_platform,
            paths,
            target,
            state.stop.clone(),
        )
        .await
        .map_err(plain_graphql::Error::new)
    }

    async fn extract_zip_archive(
        &self,
        ctx: &Context<'_>,
        path: String,
        target: String,
    ) -> plain_graphql::Result<String> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let state = ctx.data::<crate::server::ServerState>()?;
        plain_rs::features::services::files::archive_operations::unpack(
            &state.file_platform,
            path,
            target,
            state.stop.clone(),
        )
        .await
        .map_err(plain_graphql::Error::new)
    }

    async fn cache_zip_entry(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> plain_graphql::Result<String> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let services = ctx.data::<plain_rs::runtime::Runtime>()?;
        let path = services
            .archives
            .preview(&services.file_platform, path)
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(path.to_string_lossy().into_owned())
    }

    async fn create_dir(&self, ctx: &Context<'_>, path: String) -> plain_graphql::Result<File> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let record = plain_rs::features::services::files::writes::execute(
            ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?,
            plain_libs::filesystem::writes::Operation::CreateDirectory { path },
        )
        .await
        .map_err(plain_graphql::Error::new)?;
        Ok(written_file(record))
    }

    async fn rename_file(
        &self,
        ctx: &Context<'_>,
        path: String,
        name: String,
    ) -> plain_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(ctx
            .data::<Arc<plain_rs::features::services::files::file_tasks::FileTasks>>()?
            .rename(&super::tasks::caller(ctx)?, path, name)
            .await?)
    }

    async fn write_text_file(
        &self,
        ctx: &Context<'_>,
        path: String,
        content: String,
        overwrite: bool,
    ) -> plain_graphql::Result<File> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let record = plain_rs::features::services::files::writes::execute(
            ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?,
            plain_libs::filesystem::writes::Operation::WriteText {
                path,
                content,
                overwrite,
            },
        )
        .await
        .map_err(plain_graphql::Error::new)?;
        Ok(written_file(record))
    }

    async fn copy_file_to_directory(
        &self,
        ctx: &Context<'_>,
        src: String,
        directory: String,
        name: String,
    ) -> plain_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        if (src.starts_with("content://") || src.starts_with("ph://"))
            && !public_gate::is_local(ctx)
        {
            return Err(plain_graphql::Error::new("no_permission"));
        }
        Ok(plain_rs::features::services::files::named_copy::copy(
            ctx.data::<crate::server::ServerState>()?,
            super::tasks::caller(ctx)?,
            src,
            directory,
            name,
        )
        .await?)
    }

    async fn copy_file(
        &self,
        ctx: &Context<'_>,
        src: String,
        dst: String,
        overwrite: bool,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        transfer(ctx, "COPY", src, dst, overwrite).await
    }

    async fn move_file(
        &self,
        ctx: &Context<'_>,
        src: String,
        dst: String,
        overwrite: bool,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        transfer(ctx, "MOVE", src, dst, overwrite).await
    }

    async fn delete_chunks(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let base = upload_base(ctx).await?;
        ctx.data_unchecked::<std::sync::Arc<plain_rs::features::files::uploads::Runtime>>()
            .delete(&base, &file_id)
            .await
            .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }

    /// Starts the merge and returns immediately; completion arrives over the
    /// websocket, and `mergeStatus` is the polling fallback for a lost event.
    async fn merge_chunks(
        &self,
        ctx: &Context<'_>,
        file_id: String,
        total_chunks: i32,
        path: String,
        replace: bool,
        total_size: plain_rs::infrastructure::types::Long,
    ) -> plain_graphql::Result<MergeTask> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let platform =
            ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?;
        let path = platform.resolve_reference(&path)?;
        let kind = if path.starts_with("content://") {
            if !replace {
                return Err("Resource upload requires replacement".into());
            }
            let host = ctx
                .data::<Arc<plain_rs::infrastructure::host::bridge::Host>>()?
                .clone();
            let destination = path.clone();
            plain_rs::features::files::uploads::Kind::Resource {
                name: path.rsplit('/').next().unwrap_or_default().to_owned(),
                write: Arc::new(move |source| {
                    let host = host.clone();
                    let destination = destination.clone();
                    Box::pin(async move {
                        plain_rs::features::services::files::resources::write(
                            &host,
                            &source,
                            &destination,
                        )
                        .await
                        .map_err(plain_libs::anyhow::Error::msg)
                    })
                }),
            }
        } else {
            if path.contains("://") {
                return Err("Unsupported upload destination".into());
            }
            platform.authorize(&path, true).await?;
            plain_rs::features::files::uploads::Kind::File {
                path: path.into(),
                replace,
            }
        };
        start_merge(ctx, file_id, total_chunks, total_size.0, kind).await
    }

    async fn merge_app_file_chunks(
        &self,
        ctx: &Context<'_>,
        file_id: String,
        total_chunks: i32,
        file_name: String,
        total_size: plain_rs::infrastructure::types::Long,
    ) -> plain_graphql::Result<MergeTask> {
        start_merge(
            ctx,
            file_id,
            total_chunks,
            total_size.0,
            plain_rs::features::files::uploads::Kind::AppFile { name: file_name },
        )
        .await
    }
}

#[derive(Default)]
pub struct MediaItemsMutation;

#[Object]
impl MediaItemsMutation {
    /// A blank query would match the whole library, and a bulk delete is
    /// not something a client should be able to aim at everything by
    /// accident — it is refused before the platform is asked.
    async fn delete_media_items(
        &self,
        ctx: &Context<'_>,
        r#type: MediaDataType,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        require_explicit_query(&query)?;
        require_narrowing_media(ctx, r#type, &query).await?;
        let affected = media_action(ctx, "delete", r#type, &query, Value::Null).await?;
        Ok(ActionResult {
            affected_count: affected,
        })
    }

    async fn trash_media_items(
        &self,
        ctx: &Context<'_>,
        r#type: MediaDataType,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        require_explicit_query(&query)?;
        require_narrowing_media(ctx, r#type, &query).await?;
        let affected = media_action(ctx, "trash", r#type, &query, Value::Null).await?;
        Ok(ActionResult {
            affected_count: affected,
        })
    }

    async fn restore_media_items(
        &self,
        ctx: &Context<'_>,
        r#type: MediaDataType,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        require_explicit_query(&query)?;
        require_narrowing_media(ctx, r#type, &query).await?;
        let affected = media_action(ctx, "restore", r#type, &query, Value::Null).await?;
        Ok(ActionResult {
            affected_count: affected,
        })
    }

    async fn move_media_items(
        &self,
        ctx: &Context<'_>,
        r#type: MediaDataType,
        query: String,
        dest_dir: String,
    ) -> plain_graphql::Result<ActionResult> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        require_explicit_query(&query)?;
        require_narrowing_media(ctx, r#type, &query).await?;
        let affected = media_action(ctx, "move", r#type, &query, json!(dest_dir)).await?;
        Ok(ActionResult {
            affected_count: affected,
        })
    }
}

fn require_explicit_query(query: &str) -> plain_graphql::Result<()> {
    if query.trim().is_empty() {
        return Err(plain_graphql::Error::new("explicit query required"));
    }
    Ok(())
}

/// Refuses a media action whose query builds no clause, which would resolve to
/// every row of the library. Blank is already the caller's business; this is
/// the other half — a query can be non-empty and still name nothing that
/// narrows, because the fields it carries are ones this provider does not act
/// on.
async fn require_narrowing_media(
    ctx: &Context<'_>,
    r#type: MediaDataType,
    query: &str,
) -> plain_graphql::Result<()> {
    let provider = match r#type {
        MediaDataType::AUDIO => Provider::Audio,
        MediaDataType::VIDEO => Provider::Video,
        MediaDataType::IMAGE => Provider::Image,
        MediaDataType::DOC => Provider::Doc,
    };
    provider_plan::require_narrowing(ctx.data_unchecked::<Arc<Db>>(), provider, query)
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
}

async fn transfer(
    ctx: &Context<'_>,
    kind: &str,
    src: String,
    dst: String,
    overwrite: bool,
) -> plain_graphql::Result<bool> {
    let kind = match kind {
        "COPY" => plain_rs::features::files::tasks::FileTaskType::Copy,
        "MOVE" => plain_rs::features::files::tasks::FileTaskType::Move,
        _ => return Err("invalid file task type".into()),
    };
    Ok(ctx
        .data::<Arc<plain_rs::features::services::files::file_tasks::FileTasks>>()?
        .execute(
            &super::tasks::caller(ctx)?,
            kind,
            vec![plain_rs::features::files::tasks::FileTaskOp {
                src,
                dst,
                overwrite,
            }],
        )
        .await?
        .status
        == plain_rs::features::files::tasks::FileTaskStatus::Done)
}

async fn media_action(
    ctx: &Context<'_>,
    action: &str,
    r#type: MediaDataType,
    query: &str,
    dest: Value,
) -> plain_graphql::Result<i32> {
    public_gate::require(
        ctx,
        prefs(ctx),
        &[crate::graphql::resolvers::media::public::access_feature(
            r#type.data_type(),
        )],
    )?;
    use plain_rs::features::media::actions::Action;
    let action = match action {
        "delete" => Action::Delete,
        "trash" => Action::Trash,
        "restore" => Action::Restore,
        "move" => Action::Move,
        _ => return Err("invalid media action".into()),
    };
    let data_type = match r#type {
        MediaDataType::AUDIO => plain_rs::features::enums::DataType::Audio,
        MediaDataType::VIDEO => plain_rs::features::enums::DataType::Video,
        MediaDataType::IMAGE => plain_rs::features::enums::DataType::Image,
        MediaDataType::DOC => plain_rs::features::enums::DataType::Doc,
    };
    ctx.data::<Arc<plain_rs::features::services::files::media_actions::MediaActions>>()?
        .run_query(data_type, action, query, dest.as_str().unwrap_or_default())
        .await
        .map_err(Into::into)
}

fn written_file(record: plain_libs::filesystem::record::FileRecord) -> File {
    File {
        permission: record.permission,
        file_id: (!record.file_id.is_empty()).then(|| record.file_id.into()),
        name: record.name,
        path: record.path,
        created_at: record
            .created_at
            .and_then(plain_libs::chrono::DateTime::from_timestamp_millis)
            .map(plain_rs::infrastructure::types::Instant),
        updated_at: plain_rs::infrastructure::types::Instant(
            plain_libs::chrono::DateTime::from_timestamp_millis(record.updated_at).unwrap_or_default(),
        ),
        size: Long(record.size),
        is_dir: record.is_dir,
        child_count: record.child_count,
    }
}

fn merge_task(value: &Value) -> MergeTask {
    MergeTask {
        status: MergeTaskStatus::parse(&text(value, "status")),
        value: value["value"].as_str().map(str::to_string),
        merged_size: value["mergedSize"].as_i64().map(Long),
        error: value["error"].as_str().map(str::to_string),
    }
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<plain_rs::features::prefs::Prefs> {
    ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/files/operations.rs"]
mod tests;

async fn upload_base(ctx: &Context<'_>) -> plain_graphql::Result<std::path::PathBuf> {
    Ok(ctx
        .data::<Arc<plain_rs::features::services::files::upload_chunks::UploadChunks>>()?
        .directory()
        .to_path_buf())
}
async fn start_merge(
    ctx: &Context<'_>,
    id: String,
    count: i32,
    size: i64,
    kind: plain_rs::features::files::uploads::Kind,
) -> plain_graphql::Result<MergeTask> {
    let base = upload_base(ctx).await?;
    let store = ctx
        .data_unchecked::<std::sync::Arc<plain_rs::features::files::app_files::FileStore>>()
        .clone();
    let runtime = ctx
        .data_unchecked::<std::sync::Arc<plain_rs::features::files::uploads::Runtime>>()
        .clone();
    let events = ctx
        .data_unchecked::<tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>>()
        .clone();
    let value = runtime
        .start(store, base, id, count, size, kind, events)
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
    Ok(merge_task(&value))
}

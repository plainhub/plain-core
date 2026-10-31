use crate::graphql::resolvers::app::capability_types::DlnaRenderer;
use crate::graphql::resolvers::media::types::MediaDataType;
use crate::server::ServerState;
use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use plain_rs::services::casting::dlna_sender_runtime as sender;
use plain_rs::services::casting::dlna_sender_runtime::Command;
use plain_rs::types::Long;

#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderDevice {
    id: async_graphql::ID,
    host_address: String,
    name: String,
    location: String,
}
impl From<sender::Device> for DlnaSenderDevice {
    fn from(row: sender::Device) -> Self {
        Self {
            id: row.id.into(),
            host_address: row.host_address,
            name: row.name,
            location: row.location,
        }
    }
}
#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderMedia {
    path: String,
    title: String,
    album_art: String,
    artist: String,
    duration_ms: Long,
    audio: bool,
}
impl From<sender::Item> for DlnaSenderMedia {
    fn from(row: sender::Item) -> Self {
        Self {
            path: row.path,
            title: row.title,
            album_art: row.album_art,
            artist: row.artist,
            duration_ms: Long(row.duration_ms),
            audio: row.audio,
        }
    }
}
#[derive(InputObject)]
struct DlnaMediaInput {
    path: String,
    title: String,
    #[graphql(default)]
    album_art: String,
    #[graphql(default)]
    artist: String,
    #[graphql(default)]
    duration_ms: Long,
    #[graphql(default)]
    audio: bool,
}
impl From<DlnaMediaInput> for sender::Item {
    fn from(row: DlnaMediaInput) -> Self {
        Self {
            path: row.path,
            title: row.title,
            album_art: row.album_art,
            artist: row.artist,
            duration_ms: row.duration_ms.0,
            audio: row.audio,
        }
    }
}
#[derive(SimpleObject, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DlnaSenderState {
    version: Long,
    devices: Vec<DlnaSenderDevice>,
    current_device: Option<DlnaSenderDevice>,
    items: Vec<DlnaSenderMedia>,
    current_uri: String,
    playing: bool,
    progress_ms: Long,
    duration_ms: Long,
    supports_callback: bool,
}
impl TryFrom<sender::Snapshot> for DlnaSenderState {
    type Error = async_graphql::Error;
    fn try_from(row: sender::Snapshot) -> Result<Self> {
        Ok(Self {
            version: Long(row.version.try_into()?),
            devices: row.devices.into_iter().map(Into::into).collect(),
            current_device: row.current_device.map(Into::into),
            items: row.items.into_iter().map(Into::into).collect(),
            current_uri: row.current_uri,
            playing: row.playing,
            progress_ms: Long(row.progress_ms),
            duration_ms: Long(row.duration_ms),
            supports_callback: row.supports_callback,
        })
    }
}
async fn command(ctx: &Context<'_>, command: Command) -> Result<DlnaSenderState> {
    sender::run(ctx.data::<ServerState>()?, command)
        .await
        .map_err(async_graphql::Error::new)?
        .try_into()
}
#[derive(Default)]
pub struct DlnaSenderQuery;
#[Object]
impl DlnaSenderQuery {
    async fn dlna_sender_state(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        ctx.data::<ServerState>()?.cast.snapshot().try_into()
    }

    async fn dlna_renderers(&self, ctx: &Context<'_>) -> Result<Vec<DlnaRenderer>> {
        let state = ctx.data::<ServerState>()?;
        let devices = state
            .cast
            .devices
            .lock()
            .map_err(|_| async_graphql::Error::new("DLNA device state unavailable"))?;
        let mut rows = devices
            .values()
            .filter_map(|device| device.to_renderer())
            .map(|row| DlnaRenderer {
                udn: row.udn,
                name: row.name,
                manufacturer: (!row.manufacturer.is_empty()).then_some(row.manufacturer),
                model_name: (!row.model_name.is_empty()).then_some(row.model_name),
                location: row.location,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.udn.cmp(&b.udn)));
        Ok(rows)
    }
}
#[derive(Default)]
pub struct DlnaSenderMutation;
#[Object]
impl DlnaSenderMutation {
    async fn dlna_cast(
        &self,
        ctx: &Context<'_>,
        renderer_udn: String,
        url: String,
        title: String,
        mime: String,
        #[graphql(name = "type")] media_type: MediaDataType,
    ) -> Result<bool> {
        if media_type == MediaDataType::DOC {
            return Err("dlna_cast_doc_unsupported".into());
        }
        let state = ctx.data::<ServerState>()?;
        let runtime = state.cast.clone();
        let _guard = runtime.operations.lock().await;
        let result = async {
            sender::execute(state, &runtime, Command::Select { id: renderer_udn }).await?;
            plain_rs::services::casting::dlna_sender_playback::cast_with_mime(
                state,
                sender::Item {
                    path: url,
                    title,
                    audio: media_type == MediaDataType::AUDIO,
                    ..Default::default()
                },
                false,
                Some(mime),
            )
            .await
        }
        .await;
        runtime.publish(state);
        result.map_err(async_graphql::Error::new)?;
        Ok(true)
    }
    async fn start_dlna_scan(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::StartScan).await
    }
    async fn stop_dlna_scan(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::StopScan).await
    }
    async fn play_dlna(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::Play).await
    }
    async fn pause_dlna(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::Pause).await
    }
    async fn stop_dlna(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::Stop).await
    }
    async fn exit_dlna(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::Exit).await
    }
    async fn clear_dlna_items(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        command(ctx, Command::Clear).await
    }
    async fn select_dlna_device(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> Result<DlnaSenderState> {
        command(ctx, Command::Select { id: id.to_string() }).await
    }
    async fn cast_dlna_media(
        &self,
        ctx: &Context<'_>,
        item: DlnaMediaInput,
    ) -> Result<DlnaSenderState> {
        command(ctx, Command::Cast { item: item.into() }).await
    }
    async fn add_dlna_item(
        &self,
        ctx: &Context<'_>,
        item: DlnaMediaInput,
    ) -> Result<DlnaSenderState> {
        command(ctx, Command::Add { item: item.into() }).await
    }
    async fn remove_dlna_item(&self, ctx: &Context<'_>, path: String) -> Result<DlnaSenderState> {
        command(ctx, Command::Remove { path }).await
    }
    async fn seek_dlna(&self, ctx: &Context<'_>, position_ms: Long) -> Result<DlnaSenderState> {
        command(
            ctx,
            Command::Seek {
                position_ms: position_ms.0,
            },
        )
        .await
    }
    async fn remove_dlna_item_at(&self, ctx: &Context<'_>, index: i32) -> Result<DlnaSenderState> {
        command(
            ctx,
            Command::RemoveAt {
                index: index.try_into()?,
            },
        )
        .await
    }
    async fn reorder_dlna_items(
        &self,
        ctx: &Context<'_>,
        from: i32,
        to: i32,
    ) -> Result<DlnaSenderState> {
        command(
            ctx,
            Command::Reorder {
                from: from.try_into()?,
                to: to.try_into()?,
            },
        )
        .await
    }
}

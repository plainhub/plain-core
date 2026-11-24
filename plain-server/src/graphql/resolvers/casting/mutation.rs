use super::DlnaSenderState;
use crate::server::ServerState;
use plain_graphql::{Context, InputObject, Object, Result};
use plain_rs::features::services::casting::dlna_sender_runtime as sender;
use plain_rs::features::services::casting::dlna_sender_runtime::{Command, DlnaCastType};
use plain_rs::infrastructure::types::Long;

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
    #[graphql(name = "type")]
    media_type: DlnaCastType,
}

async fn command(ctx: &Context<'_>, command: Command) -> Result<DlnaSenderState> {
    sender::run(ctx.data::<ServerState>()?, command)
        .await
        .map_err(plain_graphql::Error::new)?
        .try_into()
}

impl From<DlnaMediaInput> for sender::Item {
    fn from(row: DlnaMediaInput) -> Self {
        Self {
            path: row.path,
            title: row.title,
            album_art: row.album_art,
            artist: row.artist,
            duration_ms: row.duration_ms.0,
            media_type: row.media_type,
        }
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
        #[graphql(name = "type")] media_type: DlnaCastType,
    ) -> Result<bool> {
        let state = ctx.data::<ServerState>()?;
        let runtime = state.cast.clone();
        let interruption = runtime.media.interruption();
        let _guard = runtime.operations.lock().await;
        if interruption != runtime.media.interruption() {
            return Err("Cast canceled".into());
        }
        let result = async {
            sender::execute(state, &runtime, Command::Select { id: renderer_udn }).await?;
            plain_rs::features::services::casting::dlna_sender_playback::cast_with_mime(
                state,
                sender::Item {
                    path: url,
                    title,
                    media_type,
                    ..Default::default()
                },
                false,
                Some(mime),
            )
            .await
        }
        .await;
        runtime.publish(state);
        result.map_err(plain_graphql::Error::new)?;
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
        id: plain_graphql::ID,
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

use super::{Share, record, state};
use plain_graphql::{Context, ID, InputObject, Object};
use plain_rs::features::files::shares;
use plain_rs::features::files::shares::{Service, SharePath};
use plain_rs::infrastructure::types::Instant;

#[derive(InputObject)]
struct SharePathInput {
    virtual_path: String,
    real_path: String,
    is_dir: bool,
}

#[derive(InputObject)]
struct ShareInput {
    name: String,
    password: String,
    expires_at: Option<Instant>,
    read_only: bool,
    data: Vec<SharePathInput>,
}

impl From<ShareInput> for shares::Input {
    fn from(input: ShareInput) -> Self {
        Self {
            name: input.name,
            password: input.password,
            expires_at: input.expires_at.map(|i| {
                i.0.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
            }),
            read_only: input.read_only,
            data: input
                .data
                .into_iter()
                .map(|path| SharePath {
                    virtual_path: path.virtual_path,
                    real_path: path.real_path,
                    is_dir: path.is_dir,
                })
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct ShareHostMutation;

#[Object]
impl ShareHostMutation {
    async fn create_share(
        &self,
        ctx: &Context<'_>,
        input: ShareInput,
    ) -> plain_graphql::Result<Share> {
        save(ctx, None, input).await
    }
    async fn update_share(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: ShareInput,
    ) -> plain_graphql::Result<Share> {
        save(ctx, Some(id), input).await
    }
    fn delete_share(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        Ok(state(ctx)?.db.share_delete(&id)? > 0)
    }
}

async fn save(
    ctx: &Context<'_>,
    id: Option<ID>,
    input: ShareInput,
) -> plain_graphql::Result<Share> {
    let state = state(ctx)?;
    let input: shares::Input = input.into();
    state
        .file_platform
        .authorize_metadata_paths(
            &input
                .data
                .iter()
                .map(|path| path.real_path.clone())
                .collect::<Vec<_>>(),
        )
        .await?;
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let row = tokio::task::spawn_blocking(move || match id {
        Some(id) => service.update(&id, input),
        None => service.create(input),
    })
    .await??;
    record(state, row).await
}

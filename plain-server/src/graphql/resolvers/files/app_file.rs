use plain_graphql::{ComplexObject, Context, Object, SimpleObject};

use std::sync::Arc;

use crate::graphql::types::parse_instant;
use plain_rs::features::chat::attachments::app_file_store::display_name;
use plain_rs::features::chat::attachments::app_file_store::file_name_map;
use plain_rs::infrastructure::db::DAppFile;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Instant;
use plain_rs::infrastructure::types::Long;

#[derive(SimpleObject)]
#[graphql(name = "AppFile")]
#[graphql(complex)]
pub struct AppFile {
    pub id: String,
    pub size: Long,
    pub mime_type: String,
    #[graphql(skip)]
    pub real_path: String,
    pub file_name: String,
    #[graphql(skip)]
    pub created_at: String,
    #[graphql(skip)]
    pub updated_at: String,
}

#[ComplexObject]
impl AppFile {
    fn real_path(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        Ok(ctx
            .data::<Arc<plain_rs::features::files::app_files::FileStore>>()?
            .stored_path(&self.real_path)
            .map_err(plain_graphql::Error::new)?
            .to_string_lossy()
            .into_owned())
    }

    fn created_at(&self) -> plain_graphql::Result<Instant> {
        parse_instant(&self.created_at)
    }

    fn updated_at(&self) -> plain_graphql::Result<Instant> {
        parse_instant(&self.updated_at)
    }
}

impl AppFile {
    pub fn from_dappfile(f: DAppFile, file_name: String) -> Self {
        Self {
            id: f.id,
            size: Long(f.size_bytes),
            mime_type: f.mime_type,
            real_path: f.real_path,
            file_name,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}

#[derive(Default)]
pub struct AppFileQuery;

#[Object]
impl AppFileQuery {
    fn app_files(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<AppFile>> {
        crate::graphql::resolvers::common::validate_pagination(offset, limit)?;
        let db = ctx.data::<Arc<Db>>()?;
        let files = db.app_file_items(offset, limit, &query)?;
        let names = file_name_map(&db.get_all_chats());
        Ok(files
            .into_iter()
            .map(|file| {
                let name = display_name(&file, &names);
                AppFile::from_dappfile(file, name)
            })
            .collect())
    }

    fn app_file_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        Ok(ctx.data::<Arc<Db>>()?.app_file_count(&query)?)
    }
}

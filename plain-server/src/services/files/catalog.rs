use super::{media_catalog, media_catalog_facts};
use crate::graphql::resolvers::files::types::FileSortBy;
use crate::host::bridge::Host;
use plain_rs::{db::Db, enums::DataType};
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) enum MediaCatalog {
    Native(Arc<Db>),
    Platform { db: Arc<Db>, host: Arc<Host> },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MediaVolumeRevision {
    volume: String,
    version: String,
    generation: i64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MediaPage {
    pub rows: Vec<Value>,
    pub revision_before: Option<Vec<MediaVolumeRevision>>,
    pub revision_after: Option<Vec<MediaVolumeRevision>>,
}

impl MediaPage {
    fn validate(mut self) -> Result<Self, String> {
        for volumes in [&mut self.revision_before, &mut self.revision_after]
            .into_iter()
            .flatten()
        {
            if volumes.len() > 64 {
                return Err("too many media volumes".into());
            }
            volumes.sort_by(|a, b| a.volume.cmp(&b.volume));
            if volumes.iter().any(|volume| {
                volume.volume.is_empty() || volume.version.is_empty() || volume.generation < 0
            }) || volumes
                .windows(2)
                .any(|pair| pair[0].volume == pair[1].volume)
            {
                return Err("invalid media volume revision".into());
            }
        }
        Ok(self)
    }
}

impl MediaCatalog {
    pub(crate) fn new(db: Arc<Db>, host: Arc<Host>) -> Self {
        if host.uses_system_media_catalog() {
            Self::Platform { db, host }
        } else {
            Self::Native(db)
        }
    }

    pub(crate) async fn rows(
        &self,
        kind: DataType,
        query: &str,
        sort: FileSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Value>, String> {
        match self {
            Self::Platform { db, host } => {
                let facts = host.call(
                    "systemMediaRows",
                    platform_params(db, kind, query, json!({"dataType":kind,"sortBy":sort.as_str(),"limit":limit,"offset":offset}))?,
                ).await?;
                serde_json::from_value(facts).map_err(|error| error.to_string())
            }
            Self::Native(db) => {
                let db = db.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::rows(&db, kind, &query, sort, limit, offset)
                        .map_err(|error| error.to_string())?
                        .iter()
                        .map(media_catalog_facts::row_facts)
                        .collect()
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub(crate) async fn sampled_rows(
        &self,
        kind: DataType,
        query: &str,
        sort: FileSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<MediaPage, String> {
        match self {
            Self::Platform { db, host } => {
                let facts = host.call("systemMediaRows", platform_params(
                    db, kind, query,
                    json!({"dataType":kind,"sortBy":sort.as_str(),"limit":limit,"offset":offset,"includeRevision":true}),
                )?).await?;
                serde_json::from_value::<MediaPage>(facts)
                    .map_err(|error| error.to_string())?
                    .validate()
            }
            Self::Native(_) => Ok(MediaPage {
                rows: self.rows(kind, query, sort, limit, offset).await?,
                revision_before: None,
                revision_after: None,
            }),
        }
    }

    pub(crate) async fn image_matches(
        &self,
        query: &str,
        ids: &[String],
        include_text: bool,
    ) -> Result<Vec<Value>, String> {
        let db = match self {
            Self::Native(db) | Self::Platform { db, .. } => db,
        };
        let plan =
            image_search_plan(db, query, ids, include_text).map_err(|error| error.to_string())?;
        if plan.ids.is_empty() {
            return Ok(Vec::new());
        }
        let limit = plan.ids.len() as u32;
        match self {
            Self::Platform { host, .. } => {
                let facts = host.call(
                    "systemMediaRows",
                    json!({"dataType":DataType::Image,"sortBy":"NAME_ASC","limit":limit,"offset":0,"plan":plan}),
                ).await?;
                serde_json::from_value(facts).map_err(|error| error.to_string())
            }
            Self::Native(db) => {
                let db = db.clone();
                tokio::task::spawn_blocking(move || {
                    media_catalog::rows_with_plan(
                        &db,
                        DataType::Image,
                        plan,
                        FileSortBy::NameAsc,
                        limit,
                        0,
                    )
                    .map_err(|error| error.to_string())?
                    .iter()
                    .map(media_catalog_facts::row_facts)
                    .collect()
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub(crate) async fn count(&self, kind: DataType, query: &str) -> Result<i64, String> {
        match self {
            Self::Platform { db, host } => host
                .call(
                    "systemMediaCount",
                    platform_params(db, kind, query, json!({"dataType":kind}))?,
                )
                .await?
                .as_i64()
                .ok_or_else(|| "Invalid media count".to_owned()),
            Self::Native(db) => {
                let db = db.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::count(&db, kind, &query).map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub(crate) async fn ids(
        &self,
        kind: DataType,
        query: &str,
        from_trash: bool,
    ) -> Result<Vec<String>, String> {
        match self {
            Self::Platform { db, host } => serde_json::from_value(
                host.call("systemMediaIdsFacts", {
                    let mut params = platform_params(db, kind, query, json!({"dataType":kind}))?;
                    if from_trash {
                        params["plan"]["trash"] = json!(true);
                    }
                    params
                })
                .await?,
            )
            .map_err(|error| error.to_string()),
            Self::Native(db) => {
                let db = db.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::ids(&db, kind, &query, from_trash)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub(crate) async fn document_extensions(
        &self,
        query: &str,
    ) -> Result<Vec<(String, i32)>, String> {
        match self {
            Self::Native(db) => {
                let db = db.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::document_extensions(&db, &query)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
            Self::Platform { db, host } => {
                let mut counts = std::collections::BTreeMap::new();
                let mut offset = 0;
                let page_size = 512;
                loop {
                    let facts = host.call("systemMediaRows", platform_params(db, DataType::Doc, query,
                        json!({"dataType":DataType::Doc,"sortBy":"NAME_ASC","pathsOnly":true,"limit":page_size,"offset":offset}))?).await?;
                    let paths: Vec<String> =
                        serde_json::from_value(facts).map_err(|error| error.to_string())?;
                    for path in &paths {
                        media_catalog::add_document_extension(&mut counts, path);
                    }
                    if paths.len() < page_size {
                        break;
                    }
                    offset += paths.len();
                }
                Ok(counts.into_iter().collect())
            }
        }
    }

    pub(crate) async fn buckets(&self, kind: DataType) -> Result<Value, String> {
        match self {
            Self::Platform { host, .. } => {
                host.call("systemMediaBucketItemFacts", json!({"dataType":kind,"plan":super::media_buckets::platform_plan(kind).map_err(|error| error.to_string())?}))
                    .await
            }
            Self::Native(db) => {
                let db = db.clone();
                tokio::task::spawn_blocking(move || {
                    media_catalog::bucket_item_facts(&db, kind)
                        .map(Value::Array)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }
}

fn image_search_plan(
    db: &Db,
    query: &str,
    ids: &[String],
    include_text: bool,
) -> anyhow::Result<crate::host::query::Plan> {
    use crate::host::query::{self, Provider};
    use std::collections::HashSet;

    let mut fields = query::fields(db, query)?;
    if !include_text {
        fields.retain(|field| field.name != "text");
    }
    let mut plan = query::plan(
        Provider::Image,
        &fields,
        &Value::Null,
        None,
        query.is_empty(),
    )?;
    let allowed = plan.ids.iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let matches = ids
        .iter()
        .filter(|id| plan.ids_column.is_none() || allowed.contains(id))
        .filter(|id| seen.insert(*id))
        .cloned()
        .collect();
    plan.ids_column = Some("_id".into());
    plan.ids = matches;
    Ok(plan)
}

fn platform_params(db: &Db, kind: DataType, query: &str, params: Value) -> Result<Value, String> {
    crate::host::query::params(
        db,
        crate::host::query::media_provider(kind).map_err(|error| error.to_string())?,
        query,
        params,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/catalog.rs"]
mod tests;

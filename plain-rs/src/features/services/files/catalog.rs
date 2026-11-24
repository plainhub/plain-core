use crate::features::enums::DataType;
use crate::features::prefs::Prefs;
use crate::features::prefs::settings::MediaSortBy;
use crate::features::services::files::media_catalog;
use crate::features::services::files::media_catalog_facts;
use crate::infrastructure::db::Db;
use crate::infrastructure::host::bridge::Host;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

pub enum MediaCatalog {
    Native(Arc<Db>, Arc<Prefs>),
    Platform {
        db: Arc<Db>,
        host: Arc<Host>,
        prefs: Arc<Prefs>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaVolumeRevision {
    volume: String,
    version: String,
    generation: i64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaPage {
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
    pub fn new(db: Arc<Db>, host: Arc<Host>, prefs: Arc<Prefs>) -> Self {
        if host.uses_system_media_catalog() {
            Self::Platform { db, host, prefs }
        } else {
            Self::Native(db, prefs)
        }
    }

    fn filesystem_trash_catalog(&self, query: &str, from_trash: bool) -> Option<Self> {
        let Self::Platform { db, host, prefs } = self else {
            return None;
        };
        if !host.startup().system_media_trash
            && (from_trash || super::provider_query::requests_media_trash(query))
        {
            Some(Self::Native(db.clone(), prefs.clone()))
        } else {
            None
        }
    }

    pub async fn rows(
        &self,
        kind: DataType,
        query: &str,
        sort: MediaSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Value>, String> {
        match self.filesystem_trash_catalog(query, false).as_ref().unwrap_or(self) {
            Self::Platform { db, host, prefs } => {
                let facts = host.call(
                    "systemMediaRows",
                    platform_params(db, prefs, kind, query, json!({"dataType":kind,"sortBy":sort.as_str(),"limit":limit,"offset":offset}))?,
                ).await?;
                plain_libs::serde_json::from_value(facts).map_err(|error| error.to_string())
            }
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::rows(&db, &prefs, kind, &query, sort, limit, offset)
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

    pub async fn sampled_rows(
        &self,
        kind: DataType,
        query: &str,
        sort: MediaSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<MediaPage, String> {
        match self.filesystem_trash_catalog(query, false).as_ref().unwrap_or(self) {
            Self::Platform { db, host, prefs } => {
                let facts = host.call("systemMediaRows", platform_params(
                    db, prefs, kind, query,
                    json!({"dataType":kind,"sortBy":sort.as_str(),"limit":limit,"offset":offset,"includeRevision":true}),
                )?).await?;
                plain_libs::serde_json::from_value::<MediaPage>(facts)
                    .map_err(|error| error.to_string())?
                    .validate()
            }
            Self::Native(..) => Ok(MediaPage {
                rows: self.rows(kind, query, sort, limit, offset).await?,
                revision_before: None,
                revision_after: None,
            }),
        }
    }

    pub async fn image_matches(
        &self,
        query: &str,
        ids: &[String],
        include_text: bool,
    ) -> Result<Vec<Value>, String> {
        let db = match self {
            Self::Native(db, _) | Self::Platform { db, .. } => db,
        };
        let plan =
            image_search_plan(db, query, ids, include_text).map_err(|error| error.to_string())?;
        if plan.ids.is_empty() {
            return Ok(Vec::new());
        }
        let limit = plan.ids.len() as u32;
        match self {
            Self::Platform { host, prefs, .. } => {
                let mut plan = plan;
                plan.exclude_media_directories(prefs).map_err(|error| error.to_string())?;
                let facts = host.call(
                    "systemMediaRows",
                    json!({"dataType":DataType::Image,"sortBy":"NAME_ASC","limit":limit,"offset":0,"plan":plan}),
                ).await?;
                plain_libs::serde_json::from_value(facts).map_err(|error| error.to_string())
            }
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                tokio::task::spawn_blocking(move || {
                    media_catalog::rows_with_plan(
                        &db,
                        &prefs,
                        DataType::Image,
                        plan,
                        MediaSortBy::NameAsc,
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

    pub async fn count(&self, kind: DataType, query: &str) -> Result<i64, String> {
        match self.filesystem_trash_catalog(query, false).as_ref().unwrap_or(self) {
            Self::Platform { db, host, prefs } => host
                .call(
                    "systemMediaCount",
                    platform_params(db, prefs, kind, query, json!({"dataType":kind}))?,
                )
                .await?
                .as_i64()
                .ok_or_else(|| "Invalid media count".to_owned()),
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::count(&db, &prefs, kind, &query).map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub async fn ids(
        &self,
        kind: DataType,
        query: &str,
        from_trash: bool,
    ) -> Result<Vec<String>, String> {
        match self.filesystem_trash_catalog(query, from_trash).as_ref().unwrap_or(self) {
            Self::Platform { db, host, prefs } => plain_libs::serde_json::from_value(
                host.call("systemMediaIdsFacts", {
                    let mut params = platform_params(db, prefs, kind, query, json!({"dataType":kind}))?;
                    if from_trash {
                        params["plan"]["trash"] = json!(true);
                    }
                    params
                })
                .await?,
            )
            .map_err(|error| error.to_string()),
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::ids(&db, &prefs, kind, &query, from_trash)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
        }
    }

    pub async fn document_extensions(&self, query: &str) -> Result<Vec<(String, i32)>, String> {
        match self {
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                let query = query.to_owned();
                tokio::task::spawn_blocking(move || {
                    media_catalog::document_extensions(&db, &prefs, &query)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())?
            }
            Self::Platform { db, host, prefs } => {
                let mut counts = std::collections::BTreeMap::new();
                let mut offset = 0;
                let page_size = 512;
                loop {
                    let facts = host.call("systemMediaRows", platform_params(db, prefs, DataType::Doc, query,
                        json!({"dataType":DataType::Doc,"sortBy":"NAME_ASC","pathsOnly":true,"limit":page_size,"offset":offset}))?).await?;
                    let paths: Vec<String> = plain_libs::serde_json::from_value(facts)
                        .map_err(|error| error.to_string())?;
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

    pub async fn buckets(
        &self,
        kind: DataType,
    ) -> Result<Vec<super::media_buckets::Bucket>, String> {
        match self {
            Self::Platform { host, prefs, .. } => {
                let mut plan = crate::features::services::files::media_buckets::platform_plan(kind)
                    .map_err(|error| error.to_string())?;
                plan.exclude_media_directories(prefs).map_err(|error| error.to_string())?;
                let facts = host
                    .call(
                        "systemMediaBucketItemFacts",
                        json!({"dataType":kind,"plan":plan}),
                    )
                    .await?;
                let items =
                    plain_libs::serde_json::from_value(facts).map_err(|error| error.to_string())?;
                super::media_buckets::aggregate(kind, items).map_err(|error| error.to_string())
            }
            Self::Native(db, prefs) => {
                let db = db.clone();
                let prefs = prefs.clone();
                tokio::task::spawn_blocking(move || {
                    media_catalog::buckets(&db, &prefs, kind).map_err(|error| error.to_string())
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
) -> plain_libs::anyhow::Result<crate::features::services::files::provider_query::Plan> {
    use crate::features::services::files::provider_query::{self as query, Provider};
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

fn platform_params(
    db: &Db,
    prefs: &Prefs,
    kind: DataType,
    text: &str,
    mut params: Value,
) -> Result<Value, String> {
    use crate::features::services::files::provider_query as query;
    let fields = query::fields(db, text).map_err(|error| error.to_string())?;
    let mut plan = query::plan(
        query::media_provider(kind).map_err(|error| error.to_string())?,
        &fields,
        &query::dates(&fields),
        None,
        text.is_empty(),
    )
    .map_err(|error| error.to_string())?;
    plan.exclude_media_directories(prefs).map_err(|error| error.to_string())?;
    params["plan"] = plain_libs::serde_json::to_value(plan).map_err(|error| error.to_string())?;
    Ok(params)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/catalog.rs"]
mod tests;

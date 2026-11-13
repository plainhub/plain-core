use super::domain_types::Tag;
use plain_graphql::Context;
use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::db::Db;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::oneshot;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct TagKey {
    kind: i32,
    id: String,
}

type TagResult = plain_graphql::Result<Vec<Tag>>;
type PendingTags = HashMap<TagKey, Vec<oneshot::Sender<TagResult>>>;

pub(crate) struct TagsLoader {
    db: Arc<Db>,
    pending: Mutex<PendingTags>,
    #[cfg(test)]
    pub(crate) loads: std::sync::atomic::AtomicUsize,
}

pub(crate) fn new(db: Arc<Db>) -> Arc<TagsLoader> {
    Arc::new(TagsLoader {
        db,
        pending: Mutex::new(HashMap::new()),
        #[cfg(test)]
        loads: Default::default(),
    })
}

impl TagsLoader {
    async fn load_one(self: &Arc<Self>, key: TagKey) -> TagResult {
        let (sender, receiver) = oneshot::channel();
        let start_batch = {
            let mut pending = self.pending.lock().unwrap();
            let start_batch = pending.is_empty();
            pending.entry(key).or_default().push(sender);
            start_batch
        };
        if start_batch {
            let loader = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                let pending = std::mem::take(&mut *loader.pending.lock().unwrap());
                let keys: Vec<_> = pending.keys().cloned().collect();
                let result = loader.load_batch(&keys);
                for (key, senders) in pending {
                    let tags = match &result {
                        Ok(tags) => Ok(tags[&key].clone()),
                        Err(error) => Err(error.clone()),
                    };
                    for sender in senders {
                        let _ = sender.send(tags.clone());
                    }
                }
            });
        }
        receiver
            .await
            .map_err(|_| plain_graphql::Error::new("tag batch task stopped"))?
    }

    fn load_batch(&self, keys: &[TagKey]) -> plain_graphql::Result<HashMap<TagKey, Vec<Tag>>> {
        #[cfg(test)]
        self.loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut groups: HashMap<i32, Vec<String>> = HashMap::new();
        for key in keys {
            groups.entry(key.kind).or_default().push(key.id.clone());
        }
        let mut result: HashMap<_, _> = keys.iter().cloned().map(|key| (key, Vec::new())).collect();
        for (kind, ids) in groups {
            let tags: HashMap<_, _> = plain_rs::features::tags::tags_by_type(&self.db, kind)?
                .into_iter()
                .map(|tag| (tag.id.clone(), tag))
                .collect();
            for relation in
                plain_rs::features::tags::relations_for_keys_of_kind(&self.db, &ids, kind)?
            {
                if let Some(tag) = tags.get(&relation.tag_id) {
                    result
                        .entry(TagKey {
                            kind,
                            id: relation.target_key,
                        })
                        .or_default()
                        .push(Tag::from_row(tag)?);
                }
            }
        }
        Ok(result)
    }
}

pub(crate) async fn load(ctx: &Context<'_>, kind: DataType, id: &str) -> TagResult {
    ctx.data::<Arc<TagsLoader>>()?
        .load_one(TagKey {
            kind: kind.kind(),
            id: id.to_owned(),
        })
        .await
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/tags/loader.rs"]
mod tests;

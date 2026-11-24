pub mod mutation;
pub mod query;

use crate::graphql::subscriptions::Subscription;
use mutation::Mutation;
use plain_graphql::Schema;
use query::Query;
use std::sync::Arc;

pub type PublicSchema = Schema<Query, Mutation, Subscription>;

pub(crate) fn build_with_runtime(runtime: plain_rs::runtime::Runtime) -> PublicSchema {
    Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .register_output_type::<crate::graphql::resolvers::media::item::MediaItem>()
    .data(runtime.clone())
    .data(crate::graphql::resolvers::tags::loader::new(
        runtime.db.clone(),
    ))
    .data(runtime.db)
    .data(runtime.host)
    .data(runtime.prefs)
    .data(runtime.events)
    .data(runtime.image_models)
    .data(runtime.mms)
    .data(Arc::new(plain_libs::phone::Service::default()))
    .data(runtime.media_info)
    .data(runtime.file_platform)
    .data(runtime.sms)
    .data(runtime.sms_sends)
    .data(runtime.store)
    .data(runtime.duration_cache)
    .data(runtime.duration_updates)
    .data(runtime.app_files)
    .data(runtime.shares)
    .data(runtime.media_actions)
    .data(runtime.files)
    .data(runtime.audio)
    .data(runtime.index)
    .data(runtime.pomodoro)
    .data(runtime.image_updates)
    .data(runtime.feeds)
    .data(runtime.feed_assets)
    .data(runtime.upload_chunks)
    .data(runtime.uploads)
    .data(runtime.temp_values)
    .finish()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/schemas/public/mod.rs"]
mod tests;

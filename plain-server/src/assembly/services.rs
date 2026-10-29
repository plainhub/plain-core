use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct Services {
    pub cache: plain_rs::cache_paths::CachePaths,
    pub media_info: Arc<crate::services::files::info::Info>,
    pub remote_cache: Arc<crate::services::files::remote_cache::RemoteCache>,
    pub resources: Arc<crate::host::resources::NativeResources>,
    pub logs: Arc<crate::services::logs::Logs>,
    pub device: Arc<crate::services::device::Status>,
    pub browser: Arc<crate::services::files::browser::Runtime>,
    pub file_platform: Arc<crate::services::files::platform::FilePlatform>,
    #[cfg(feature = "media")]
    pub media_scans: Arc<crate::services::files::media_scans::MediaScans>,
    pub sms_sends: Arc<crate::services::sms::send_runtime::Runtime>,
    pub sms: Arc<crate::services::sms::mutations::Mutations>,
    pub store: Arc<plain_rs::kv::LazyDb>,
    pub archives: Arc<crate::services::files::archives::Archives>,
    pub app_files: Arc<plain_rs::files::app_files::FileStore>,
    pub shares: Arc<plain_rs::files::shares::Service>,
    pub duration_updates: Arc<crate::services::files::duration_updates::Runtime>,
    pub duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    pub media_actions: Arc<crate::services::files::media_actions::MediaActions>,
    pub files: Arc<crate::services::files::file_tasks::FileTasks>,
    pub audio: Arc<crate::services::audio::host::Audio>,
    pub index: Arc<crate::services::images::index::ImageIndex>,
    pub pomodoro: Arc<crate::services::pomodoro::Service>,
    pub feeds: Arc<crate::services::feeds::SyncService>,
    pub feed_assets: Arc<crate::services::feeds::FeedAssets>,
    pub image_updates: Arc<plain_rs::media::images::editor::Updates>,
    pub upload_chunks: Arc<crate::services::files::upload_chunks::UploadChunks>,
    pub uploads: Arc<plain_rs::files::uploads::Runtime>,
    pub temp_values: crate::services::temp_values::TempValues,
}
impl Services {
    pub fn new(
        db: Arc<plain_rs::db::Db>,
        host: Arc<crate::host::bridge::Host>,
        events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
        prefs: Arc<plain_rs::prefs::Prefs>,
        directory: std::path::PathBuf,
        cache: plain_rs::cache_paths::CachePaths,
    ) -> Self {
        let path = plain_rs::data_paths::DataPaths::new(&directory).database();
        let store = Arc::new(plain_rs::kv::LazyDb::from_open(path, db.key_values()));
        let duration_cache = Arc::new(plain_rs::media::duration_cache::DurationCache::with_store(
            store.clone(),
        ));
        let resources = Arc::new(crate::host::resources::NativeResources::new(host.clone()));
        let media_info = crate::services::files::info::Info::new(
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let duration_updates = crate::services::files::duration_updates::Runtime::new(
            duration_cache.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let audio = Arc::new(crate::services::audio::host::Audio::new(
            db.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        let index = Arc::new(crate::services::images::index::ImageIndex::new(
            db.clone(),
            host.clone(),
            events.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        #[cfg(feature = "media")]
        let media_scans = Arc::new(crate::services::files::media_scans::MediaScans::new(
            events.clone(),
            cache.root().to_owned(),
        ));
        let file_platform = Arc::new(crate::services::files::platform::FilePlatform::new(
            host.clone(),
            db.clone(),
            directory.clone(),
            cache.root().to_owned(),
            events.clone(),
            #[cfg(feature = "media")]
            media_scans.clone(),
        ));
        let files = Arc::new(crate::services::files::file_tasks::FileTasks::new(
            file_platform.clone(),
            events.clone(),
            audio.clone(),
            index.clone(),
            prefs.clone(),
            duration_cache.clone(),
        ));
        let app_files = plain_rs::files::app_files::FileStore::new(db.clone(), directory.clone());
        let shares = plain_rs::files::shares::Service::new(db.clone(), prefs.clone());
        let sms = crate::services::sms::mutations::Mutations::new(
            db.clone(),
            host.clone(),
            prefs.clone(),
        );
        let sms_sends =
            crate::services::sms::send_runtime::Runtime::new(store.clone(), events.clone());
        let logs = Arc::new(crate::services::logs::Logs::new(&directory));
        let remote_cache = Arc::new(crate::services::files::remote_cache::RemoteCache::new(
            cache.root().to_owned(),
        ));
        let feed_assets = Arc::new(crate::services::feeds::FeedAssets::new(
            db.clone(),
            directory.clone(),
        ));
        let feeds = crate::services::feeds::SyncService::new(
            db.clone(),
            events.clone(),
            host.clone(),
            Some(feed_assets.clone()),
        );
        let pomodoro =
            crate::services::pomodoro::Service::new(db.clone(), prefs.clone(), events.clone());
        let image_updates = plain_rs::media::images::editor::Updates::new(events.clone());
        let uploads = Arc::new(plain_rs::files::uploads::Runtime::with_completion({
            let platform = file_platform.clone();
            move |path| {
                let platform = platform.clone();
                Box::pin(async move { platform.scan(&[path.to_string_lossy().into_owned()]).await })
            }
        }));
        let browser = crate::services::files::browser::Runtime::new(cache.root().to_owned());
        let device = Arc::new(crate::services::device::Status::new(
            directory.clone(),
            &host,
        ));
        let media_actions = Arc::new(crate::services::files::media_actions::MediaActions::new(
            #[cfg(feature = "media")]
            media_scans.clone(),
            file_platform.clone(),
            db,
            host,
            audio.clone(),
            index.clone(),
            prefs,
            events,
            duration_cache.clone(),
        ));
        Self {
            cache,
            media_info,
            remote_cache,
            resources,
            logs,
            device,
            browser,
            file_platform,
            #[cfg(feature = "media")]
            media_scans,
            sms,
            sms_sends,
            store,
            duration_cache,
            duration_updates,
            archives: Arc::new(crate::services::files::archives::Archives::new(
                directory.join("archives"),
            )),
            app_files,
            shares,
            upload_chunks: Arc::new(crate::services::files::upload_chunks::UploadChunks::new(
                directory.join("uploads"),
            )),
            media_actions,
            files,
            audio,
            index,
            pomodoro,
            feeds,
            feed_assets,
            image_updates,
            uploads,
            temp_values: Default::default(),
        }
    }
    pub(crate) fn configure_schema<
        Q: async_graphql::ObjectType + 'static,
        M: async_graphql::ObjectType + 'static,
        S: async_graphql::SubscriptionType + 'static,
    >(
        self,
        builder: async_graphql::SchemaBuilder<Q, M, S>,
        db: Arc<plain_rs::db::Db>,
        host: Arc<crate::host::bridge::Host>,
        prefs: Arc<plain_rs::prefs::Prefs>,
        events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    ) -> async_graphql::SchemaBuilder<Q, M, S> {
        builder
            .data(self.clone())
            .data(crate::graphql::resolvers::tags::loader::new(db.clone()))
            .data(db)
            .data(host)
            .data(prefs)
            .data(events)
            .data(self.media_info)
            .data(self.file_platform)
            .data(self.sms)
            .data(self.sms_sends)
            .data(self.store)
            .data(self.duration_cache)
            .data(self.duration_updates)
            .data(self.app_files)
            .data(self.shares)
            .data(self.media_actions)
            .data(self.files)
            .data(self.audio)
            .data(self.index)
            .data(self.pomodoro)
            .data(self.image_updates)
            .data(self.feeds)
            .data(self.feed_assets)
            .data(self.upload_chunks)
            .data(self.uploads)
            .data(self.temp_values)
    }
}

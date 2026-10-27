use crate::graphql::resolvers::docs::public::DocsQuery;
use crate::graphql::resolvers::images::public::ImagesQuery;
use crate::graphql::resolvers::videos::public::VideosQuery;
use crate::mobile::graphql::MobileMutation as MobileMutationImpl;
use crate::mobile::graphql::MobileQuery as MobileQueryImpl;

use crate::graphql::resolvers::audio::public::AudioMutation;
use crate::graphql::resolvers::audio::public::AudioQuery;
use crate::graphql::resolvers::chat::public::ChatMutation;
use crate::graphql::resolvers::chat::public::ChatQuery;
use crate::graphql::resolvers::clipboard::public::ClipboardMutation;
use crate::graphql::resolvers::clipboard::public::ClipboardQuery;
use crate::graphql::resolvers::database::public::DbMutation;
use crate::graphql::resolvers::database::public::DbQuery;
use crate::graphql::resolvers::device::public::DeviceMutation;
use crate::graphql::resolvers::device::public::DeviceQuery;
use crate::graphql::resolvers::downloads::public::DownloadMutation;
use crate::graphql::resolvers::favorites::favorite_folder::{
    FavoriteFolderMutation, FavoriteFolderQuery,
};
use crate::graphql::resolvers::feeds::feed::FeedMutation;
use crate::graphql::resolvers::feeds::feed::FeedQuery;
use crate::graphql::resolvers::files::operations::FileOpsMutation;
use crate::graphql::resolvers::files::operations::MediaItemsMutation;
use crate::graphql::resolvers::files::operations::UploadQuery;
use crate::graphql::resolvers::files::public::FilesQuery;
use crate::graphql::resolvers::images::public_index::ImageIndexMutation;
use crate::graphql::resolvers::images::public_index::ImageIndexQuery;
use crate::graphql::resolvers::media::public::MediaQuery;
use crate::graphql::resolvers::notes::note::NoteMutation;
use crate::graphql::resolvers::notes::note::NoteQuery;
use crate::graphql::resolvers::notifications::public::NotificationsMutation;
use crate::graphql::resolvers::notifications::public::NotificationsQuery;
use crate::graphql::resolvers::peers::public::PeersMutation;
use crate::graphql::resolvers::peers::public::PeersQuery;
use crate::graphql::resolvers::screen_mirror::public::ScreenMirrorMutation;
use crate::graphql::resolvers::screen_mirror::public::ScreenMirrorQuery;
use crate::graphql::resolvers::screen_mirror::public::SettingsMutation;
use crate::graphql::resolvers::settings::public::PrefsMutation;
use crate::graphql::resolvers::settings::public::PrefsQuery;
use crate::graphql::resolvers::tags::public::TagsMutation;
use crate::graphql::resolvers::tags::public::TagsQuery;
use crate::host::bridge::Host;
use async_graphql::{MergedObject, Schema};
use crate::graphql::subscriptions::Subscription;
use plain_rs::db::Db;
use plain_rs::prefs::Prefs;
use std::sync::Arc;

#[derive(MergedObject, Default)]
pub struct Query(
    crate::graphql::resolvers::app::cache::CacheQuery,
    crate::graphql::resolvers::peers::records::PeerRecordQuery,
    crate::graphql::resolvers::shares::queue::SharedDownloadQuery,
    crate::graphql::resolvers::downloads::queue::ChatDownloadQuery,
    crate::graphql::resolvers::discovery::mdns::MdnsQuery,
    crate::graphql::resolvers::shares::client::SharedClientQuery,
    crate::graphql::resolvers::files::trash::TrashQuery,
    crate::graphql::resolvers::casting::sender::DlnaSenderQuery,
    crate::graphql::resolvers::discovery::host::DiscoveryHostQuery,
    crate::graphql::resolvers::chat::host::ChatHostQuery,
    crate::graphql::resolvers::files::tasks::FileTaskQuery,
    crate::graphql::resolvers::media::play_history::MediaPlayHistoryQuery,
    crate::graphql::resolvers::shares::host::ShareHostQuery,
    crate::graphql::resolvers::device::sessions::SessionsQuery,
    crate::graphql::resolvers::media::scan::MediaScanQuery,
    crate::graphql::resolvers::app::platform::PlatformQuery,
    MobileQueryImpl,
    crate::graphql::resolvers::files::app_file::AppFileQuery,
    crate::graphql::resolvers::bookmarks::bookmark::BookmarkQuery,
    crate::graphql::resolvers::bookmarks::metadata::PageMetadataQuery,
    crate::graphql::resolvers::pomodoro::pomodoro::PomodoroQuery,
    crate::graphql::resolvers::images::image_editor_project::ImageEditorProjectQuery,
    NoteQuery,
    FeedQuery,
    AudioQuery,
    ChatQuery,
    PeersQuery,
    NotificationsQuery,
    ClipboardQuery,
    ScreenMirrorQuery,
    ImageIndexQuery,
    FilesQuery,
    FavoriteFolderQuery,
    MediaQuery,
    ImagesQuery,
    VideosQuery,
    DocsQuery,
    TagsQuery,
    PrefsQuery,
    DbQuery,
    UploadQuery,
    DeviceQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    crate::graphql::resolvers::app::cache::CacheMutation,
    crate::graphql::resolvers::shares::queue::SharedDownloadMutation,
    crate::graphql::resolvers::downloads::queue::ChatDownloadMutation,
    crate::graphql::resolvers::discovery::mdns::MdnsMutation,
    crate::graphql::resolvers::shares::client::SharedClientMutation,
    crate::graphql::resolvers::files::trash::TrashMutation,
    crate::graphql::resolvers::casting::sender::DlnaSenderMutation,
    crate::graphql::resolvers::audio::commands::AudioCommandsMutation,
    crate::graphql::resolvers::chat::host::ChatHostMutation,
    crate::graphql::resolvers::files::tasks::FileTaskMutation,
    crate::graphql::resolvers::media::play_history::MediaPlayHistoryMutation,
    crate::graphql::resolvers::shares::host::ShareHostMutation,
    crate::graphql::resolvers::files::app_files::AppFilesMutation,
    crate::graphql::resolvers::pomodoro::state::PomodoroControlMutation,
    crate::graphql::resolvers::device::sessions::SessionsMutation,
    crate::graphql::resolvers::app::platform::PlatformMutation,
    crate::graphql::resolvers::media::scan::MediaScanMutation,
    MobileMutationImpl,
    crate::graphql::resolvers::bookmarks::bookmark::BookmarkMutation,
    crate::graphql::resolvers::pomodoro::pomodoro::PomodoroMutation,
    crate::graphql::resolvers::images::image_editor_project::ImageEditorProjectMutation,
    NoteMutation,
    FeedMutation,
    AudioMutation,
    ChatMutation,
    PeersMutation,
    NotificationsMutation,
    ClipboardMutation,
    ScreenMirrorMutation,
    ImageIndexMutation,
    SettingsMutation,
    FavoriteFolderMutation,
    TagsMutation,
    PrefsMutation,
    DbMutation,
    FileOpsMutation,
    MediaItemsMutation,
    DeviceMutation,
    DownloadMutation,
);

pub type PublicSchema = Schema<Query, Mutation, Subscription>;

#[cfg(test)]
pub fn build(
    host: Arc<Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    directory: std::path::PathBuf,
) -> PublicSchema {
    let services = crate::assembly::services::Services::new(
        db.clone(),
        host.clone(),
        events.clone(),
        prefs.clone(),
        directory.clone(),
        plain_rs::cache_paths::CachePaths::new(&directory.join("cache")),
    );
    let models = crate::services::images::models::Runtime::new(
        directory.clone(),
        host.clone(),
        prefs.clone(),
        services.store.clone(),
        services.index.clone(),
        events.clone(),
    );
    let mobile = crate::mobile::Runtime::new(
        host.clone(),
        db.clone(),
        directory.clone(),
        services.store.clone(),
        events.clone(),
    );
    build_with_runtime(host, events, prefs, db, services, models, mobile)
}
pub(crate) fn build_with_runtime(
    host: Arc<Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    services: crate::assembly::services::Services,
    models: Arc<crate::services::images::models::Runtime>,
    mobile: Arc<crate::mobile::Runtime>,
) -> PublicSchema {
    let builder = Schema::build(Query::default(), Mutation::default(), Subscription::default())
        .extension(crate::graphql::enum_literals::EnumLiteralValidation)
        .register_output_type::<crate::graphql::resolvers::media::item::MediaItem>()
        .data(models);
    let builder = services.configure_schema(builder, db, host, prefs, events);
    let builder = crate::mobile::graphql::configure(builder, mobile);
    builder.finish()
}

#[cfg(test)]
#[path = "../../../tests/unit/graphql/schemas/public.rs"]
mod tests;

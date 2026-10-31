use crate::graphql::resolvers::docs::public::DocsQuery;
use crate::graphql::resolvers::images::public::ImagesQuery;
#[cfg(all(feature = "mobile", any(target_os = "android", target_os = "ios")))]
use crate::graphql::resolvers::mobile::MobileMutation as MobileMutationImpl;
#[cfg(all(feature = "mobile", any(target_os = "android", target_os = "ios")))]
use crate::graphql::resolvers::mobile::MobileQuery as MobileQueryImpl;
use crate::graphql::resolvers::videos::public::VideosQuery;
#[cfg(not(all(feature = "mobile", any(target_os = "android", target_os = "ios"))))]
use async_graphql::EmptyMutation as MobileMutationImpl;
#[cfg(not(all(feature = "mobile", any(target_os = "android", target_os = "ios"))))]
use async_graphql::EmptyMutation as MobileQueryImpl;

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
use crate::graphql::subscriptions::Subscription;
use async_graphql::{MergedObject, Schema};
use std::sync::Arc;

#[derive(MergedObject, Default)]
pub struct Query<Mobile: async_graphql::ObjectType + 'static = MobileQueryImpl>(
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
    Mobile,
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
pub struct Mutation<Mobile: async_graphql::ObjectType + 'static = MobileMutationImpl>(
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
    Mobile,
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

pub(crate) fn build_with_runtime(runtime: plain_rs::runtime::Runtime) -> PublicSchema {
    configure_schema(
        runtime,
        Schema::build(
            Query::default(),
            Mutation::default(),
            Subscription::default(),
        ),
    )
    .finish()
}
pub(crate) fn configure_schema<
    Q: async_graphql::ObjectType + 'static,
    M: async_graphql::ObjectType + 'static,
    S: async_graphql::SubscriptionType + 'static,
>(
    runtime: plain_rs::runtime::Runtime,
    builder: async_graphql::SchemaBuilder<Q, M, S>,
) -> async_graphql::SchemaBuilder<Q, M, S> {
    builder
        .extension(crate::graphql::enum_literals::EnumLiteralValidation)
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
        .data(Arc::new(plain_rs::phone::Service::default()))
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
}
#[cfg(test)]
#[path = "../../../tests/unit/graphql/schemas/public.rs"]
mod tests;

use crate::graphql::resolvers;
use plain_graphql::{
    OutputType,
    registry::{MetaType, Registry},
};
use std::collections::{BTreeMap, BTreeSet};

fn collect<T: OutputType>(
    root: &str,
    group: &str,
    owners: &mut BTreeMap<(String, String), String>,
    types: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let mut registry = Registry::default();
    T::create_type_info(&mut registry);
    let name = T::type_name();
    let MetaType::Object { fields, .. } = &registry.types[name.as_ref()] else {
        panic!("expected object");
    };
    for field in fields.keys() {
        assert!(
            owners
                .insert((root.into(), field.clone()), group.into())
                .is_none(),
            "duplicate root {root}.{field}"
        );
    }
    for name in registry
        .types
        .keys()
        .filter(|key| key.as_str() != name.as_ref())
    {
        types.entry(name.clone()).or_default().insert(group.into());
    }
}

fn blocks(sdl: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut lines = Vec::new();
    let mut description = false;
    for line in sdl.lines() {
        if lines.is_empty() && line.trim().is_empty() {
            continue;
        }
        lines.push(line);
        if line.matches("\"\"\"").count() % 2 == 1 {
            description = !description;
        }
        if !description
            && (line == "}"
                || ["scalar ", "union ", "directive "]
                    .iter()
                    .any(|prefix| line.starts_with(prefix)))
        {
            blocks.push(lines.join("\n") + "\n");
            lines.clear();
        }
    }
    assert!(
        lines.iter().all(|line| line.trim().is_empty()),
        "unterminated SDL block"
    );
    blocks
}

pub(super) fn files(sdl: &str) -> BTreeMap<String, String> {
    let mut owners = BTreeMap::new();
    let mut types = BTreeMap::new();
    macro_rules! group {
        ($root:literal, $group:literal, $($ty:ty),+ $(,)?) => {
            $(collect::<$ty>($root, $group, &mut owners, &mut types);)+
        };
    }
    group!("Query", "settings", resolvers::settings::query::PrefsQuery);
    group!(
        "Query",
        "key_value",
        resolvers::key_value::query::KeyValueQuery
    );
    group!(
        "Query",
        "app",
        resolvers::app::http_server::query::HttpServerQuery,
        resolvers::app::cache::query::CacheQuery,
        resolvers::app::platform::query::PlatformQuery
    );
    group!(
        "Query",
        "casting",
        resolvers::casting::query::DlnaSenderQuery
    );
    group!(
        "Query",
        "peers",
        resolvers::peers::records::query::PeerRecordQuery,
        resolvers::peers::query::PeersQuery
    );
    group!(
        "Query",
        "chat",
        resolvers::chat::host::query::ChatHostQuery,
        resolvers::chat::query::ChatQuery
    );
    group!("Query", "shares", resolvers::shares::query::ShareHostQuery);
    group!(
        "Query",
        "downloads",
        resolvers::downloads::query::DownloadQuery
    );
    group!(
        "Query",
        "discovery",
        resolvers::discovery::mdns::query::MdnsQuery,
        resolvers::discovery::query::DiscoveryHostQuery
    );
    group!(
        "Query",
        "files",
        resolvers::files::trash::query::TrashQuery,
        resolvers::files::tasks::query::FileTaskQuery,
        resolvers::files::app_files::query::AppFileQuery,
        resolvers::files::query::FilesQuery,
        resolvers::files::operations::query::UploadQuery
    );
    group!(
        "Query",
        "images",
        resolvers::images::image_editor_project::query::ImageEditorProjectQuery,
        resolvers::images::index::query::ImageIndexQuery,
        resolvers::images::query::ImagesQuery
    );
    group!("Query", "audio", resolvers::audio::query::AudioQuery);
    group!("Query", "videos", resolvers::videos::query::VideosQuery);
    group!("Query", "tags", resolvers::tags::query::TagsQuery);
    group!(
        "Query",
        "media",
        resolvers::media::scan::query::FileIndexScanQuery,
        resolvers::media::play_history::query::MediaPlayHistoryQuery,
        resolvers::media::query::MediaQuery
    );
    group!(
        "Query",
        "pomodoro",
        resolvers::pomodoro::query::PomodoroQuery
    );
    group!(
        "Query",
        "device",
        resolvers::device::sessions::query::SessionsQuery,
        resolvers::device::query::DeviceQuery
    );
    group!(
        "Query",
        "packages",
        resolvers::packages::query::PackagesQuery
    );
    group!(
        "Query",
        "contacts",
        crate::graphql::resolvers::mobile::contacts::query::ContactsQuery
    );
    group!("Query", "sms", resolvers::sms::query::SmsQuery);
    group!(
        "Query",
        "calls",
        crate::graphql::resolvers::mobile::phone_calls::query::CallsQuery
    );
    group!(
        "Query",
        "bookmarks",
        resolvers::bookmarks::query::BookmarkQuery,
        resolvers::bookmarks::metadata::query::PageMetadataQuery
    );
    group!("Query", "notes", resolvers::notes::query::NoteQuery);
    group!("Query", "feeds", resolvers::feeds::query::FeedQuery);
    group!(
        "Query",
        "notifications",
        resolvers::notifications::query::NotificationsQuery
    );
    group!(
        "Query",
        "clipboard",
        resolvers::clipboard::query::ClipboardQuery
    );
    group!(
        "Query",
        "screen_mirror",
        resolvers::screen_mirror::query::ScreenMirrorQuery
    );
    group!(
        "Query",
        "favorites",
        resolvers::favorites::query::FavoriteFolderQuery
    );
    group!("Query", "docs", resolvers::docs::query::DocsQuery);
    group!("Query", "database", resolvers::database::query::DbQuery);
    group!(
        "Mutation",
        "app",
        resolvers::app::http_server::mutation::HttpServerMutation,
        resolvers::app::cache::mutation::CacheMutation,
        resolvers::app::platform::mutation::PlatformMutation
    );
    group!(
        "Mutation",
        "settings",
        resolvers::settings::mutation::PrefsMutation
    );
    group!(
        "Mutation",
        "key_value",
        resolvers::key_value::mutation::KeyValueMutation
    );
    group!(
        "Mutation",
        "casting",
        resolvers::casting::mutation::DlnaSenderMutation
    );
    group!("Mutation", "peers", resolvers::peers::mutation::PeersMutation);
    group!(
        "Mutation",
        "shares",
        resolvers::shares::mutation::ShareHostMutation
    );
    group!(
        "Mutation",
        "downloads",
        resolvers::downloads::mutation::DownloadMutation
    );
    group!(
        "Mutation",
        "discovery",
        resolvers::discovery::mdns::mutation::MdnsMutation
    );
    group!(
        "Mutation",
        "files",
        resolvers::files::trash::mutation::TrashMutation,
        resolvers::files::tasks::mutation::FileTaskMutation,
        resolvers::files::app_files::mutation::AppFilesMutation,
        resolvers::files::operations::mutation::FileOpsMutation,
        resolvers::files::operations::mutation::MediaItemsMutation
    );
    group!(
        "Mutation",
        "images",
        resolvers::images::image_editor_project::mutation::ImageEditorProjectMutation,
        resolvers::images::index::mutation::ImageIndexMutation
    );
    group!("Mutation", "audio", resolvers::audio::mutation::AudioMutation);
    group!(
        "Mutation",
        "chat",
        resolvers::chat::host::mutation::ChatHostMutation,
        resolvers::chat::mutation::ChatMutation
    );
    group!("Mutation", "tags", resolvers::tags::mutation::TagsMutation);
    group!(
        "Mutation",
        "media",
        resolvers::media::scan::mutation::FileIndexScanMutation,
        resolvers::media::play_history::mutation::MediaPlayHistoryMutation
    );
    group!(
        "Mutation",
        "clipboard",
        resolvers::clipboard::mutation::ClipboardMutation
    );
    group!(
        "Mutation",
        "bookmarks",
        resolvers::bookmarks::mutation::BookmarkMutation
    );
    group!(
        "Mutation",
        "pomodoro",
        resolvers::pomodoro::mutation::PomodoroControlMutation,
        resolvers::pomodoro::mutation::PomodoroMutation
    );
    group!(
        "Mutation",
        "device",
        resolvers::device::sessions::mutation::SessionsMutation,
        resolvers::device::mutation::DeviceMutation
    );
    group!(
        "Mutation",
        "packages",
        resolvers::packages::mutation::PackagesMutation
    );
    group!(
        "Mutation",
        "contacts",
        crate::graphql::resolvers::mobile::contacts::mutation::ContactsMutation
    );
    group!("Mutation", "sms", resolvers::sms::mutation::SmsMutation);
    group!(
        "Mutation",
        "calls",
        crate::graphql::resolvers::mobile::phone_calls::mutation::CallsMutation
    );
    group!("Mutation", "notes", resolvers::notes::mutation::NoteMutation);
    group!("Mutation", "feeds", resolvers::feeds::mutation::FeedMutation);
    group!(
        "Mutation",
        "notifications",
        resolvers::notifications::mutation::NotificationsMutation
    );
    group!(
        "Mutation",
        "screen_mirror",
        resolvers::screen_mirror::mutation::ScreenMirrorMutation,
        resolvers::screen_mirror::mutation::SettingsMutation
    );
    group!(
        "Mutation",
        "favorites",
        resolvers::favorites::mutation::FavoriteFolderMutation
    );
    group!(
        "Mutation",
        "database",
        resolvers::database::mutation::DbMutation
    );

    let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut roots: BTreeMap<(String, String), Vec<(String, String)>> = BTreeMap::new();
    for block in blocks(sdl) {
        let declaration = block
            .lines()
            .find(|line| {
                [
                    "type ",
                    "input ",
                    "enum ",
                    "interface ",
                    "scalar ",
                    "union ",
                    "directive ",
                    "schema {",
                ]
                .iter()
                .any(|prefix| line.starts_with(prefix))
            })
            .unwrap();
        let name = declaration.split_whitespace().nth(1).unwrap_or_default();
        if name == "Subscription" && declaration.starts_with("type ") {
            let (channel, common): (Vec<_>, Vec<_>) = block
                .lines()
                .partition(|line| line.trim_start().starts_with("channelsChanged:"));
            assert_eq!(channel.len(), 1, "missing channelsChanged subscription");
            roots
                .entry(("chat_channel".into(), name.into()))
                .or_default()
                .push(("channelsChanged".into(), channel[0].into()));
            files
                .entry("common".into())
                .or_default()
                .push(common.join("\n") + "\n");
        } else if matches!(name, "Query" | "Mutation") && declaration.starts_with("type ") {
            let mut pending = Vec::new();
            let mut description = false;
            for line in block
                .lines()
                .skip_while(|line| *line != declaration)
                .skip(1)
                .take_while(|line| *line != "}")
            {
                pending.push(line);
                if line.matches("\"\"\"").count() % 2 == 1 {
                    description = !description;
                }
                let text = line.trim();
                if !description
                    && text
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && !text.starts_with("\"\"\"")
                {
                    let field = text.split(['(', ':']).next().unwrap().trim().to_owned();
                    let group = owners
                        .remove(&(name.into(), field.clone()))
                        .unwrap_or_else(|| panic!("missing owner {name}.{field}"));
                    let group = if matches!(
                        field.as_str(),
                        "chatChannels"
                            | "acceptChatChannelInvite"
                            | "addChatChannelMember"
                            | "createChatChannel"
                            | "declineChatChannelInvite"
                            | "deleteChatChannel"
                            | "leaveChatChannel"
                            | "removeChatChannelMember"
                            | "updateChatChannel"
                    ) {
                        "chat_channel".into()
                    } else {
                        group
                    };
                    roots
                        .entry((group, name.into()))
                        .or_default()
                        .push((field, pending.join("\n")));
                    pending.clear();
                }
            }
            assert!(pending.iter().all(|line| line.trim().is_empty()));
        } else {
            let group = match name {
                "ChannelChange"
                | "ChannelInvite"
                | "ChannelInviteCancellation"
                | "ChannelMemberStatus"
                | "ChatChannel"
                | "ChatChannelActions"
                | "ChatChannelMember"
                | "ChatChannelStatus" => "chat_channel",
                _ => types
                    .get(name)
                    .filter(|groups| groups.len() == 1)
                    .and_then(|groups| groups.first())
                    .map(String::as_str)
                    .unwrap_or("common"),
            };
            files.entry(group.into()).or_default().push(block);
        }
    }
    assert!(
        owners.is_empty(),
        "root fields missing from SDL: {owners:?}"
    );
    let mut defined = BTreeSet::new();
    for ((group, root), mut fields) in roots {
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        let prefix = if root != "Subscription" && defined.insert(root.clone()) {
            "type"
        } else {
            "extend type"
        };
        files.entry(group).or_default().push(format!(
            "{prefix} {root} {{\n{}\n}}\n",
            fields
                .into_iter()
                .map(|(_, text)| text)
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    files
        .into_iter()
        .map(|(group, blocks)| (format!("{group}.graphqls"), blocks.join("\n")))
        .collect()
}

pub(super) fn verify_or_write(sdl: &str) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/api");
    let files = files(sdl);
    let write = std::env::var_os("UPDATE_GRAPHQL_SCHEMAS").is_some();
    if write {
        std::fs::create_dir_all(&directory).unwrap();
    }
    let existing: BTreeSet<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    for old in existing.difference(&files.keys().cloned().collect()) {
        if write && old.ends_with(".graphqls") {
            std::fs::remove_file(directory.join(old)).unwrap();
        } else {
            panic!("unexpected schema artifact {old}");
        }
    }
    for (name, content) in files {
        let path = directory.join(name);
        if write {
            std::fs::write(path, content).unwrap();
        } else {
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                content,
                "stale {}",
                path.display()
            );
        }
    }
}

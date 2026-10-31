use crate::graphql::resolvers;
use async_graphql::{
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
    group!("Query", "settings", resolvers::settings::public::PrefsQuery);
    group!(
        "Query",
        "app",
        resolvers::app::cache::CacheQuery,
        resolvers::app::platform::PlatformQuery
    );
    group!(
        "Query",
        "casting",
        resolvers::casting::sender::DlnaSenderQuery
    );
    group!(
        "Query",
        "peers",
        resolvers::peers::records::PeerRecordQuery,
        resolvers::peers::public::PeersQuery
    );
    group!(
        "Query",
        "chat",
        resolvers::chat::host::ChatHostQuery,
        resolvers::chat::public::ChatQuery
    );
    group!(
        "Query",
        "shares",
        resolvers::shares::queue::SharedDownloadQuery,
        resolvers::shares::client::SharedClientQuery,
        resolvers::shares::host::ShareHostQuery
    );
    group!(
        "Query",
        "downloads",
        resolvers::downloads::queue::ChatDownloadQuery
    );
    group!(
        "Query",
        "discovery",
        resolvers::discovery::mdns::MdnsQuery,
        resolvers::discovery::host::DiscoveryHostQuery
    );
    group!(
        "Query",
        "files",
        resolvers::files::trash::TrashQuery,
        resolvers::files::tasks::FileTaskQuery,
        resolvers::files::app_file::AppFileQuery,
        resolvers::files::public::FilesQuery,
        resolvers::files::operations::UploadQuery
    );
    group!(
        "Query",
        "images",
        resolvers::images::image_editor_project::ImageEditorProjectQuery,
        resolvers::images::public_index::ImageIndexQuery,
        resolvers::images::public::ImagesQuery
    );
    group!("Query", "audio", resolvers::audio::public::AudioQuery);
    group!("Query", "videos", resolvers::videos::public::VideosQuery);
    group!("Query", "tags", resolvers::tags::public::TagsQuery);
    group!(
        "Query",
        "media",
        resolvers::media::scan::MediaScanQuery,
        resolvers::media::play_history::MediaPlayHistoryQuery,
        resolvers::media::public::MediaQuery
    );
    group!(
        "Query",
        "pomodoro",
        resolvers::pomodoro::pomodoro::PomodoroQuery
    );
    group!(
        "Query",
        "device",
        resolvers::device::sessions::SessionsQuery,
        resolvers::device::public::DeviceQuery
    );
    group!(
        "Query",
        "packages",
        resolvers::packages::public::PackagesQuery
    );
    group!(
        "Query",
        "contacts",
        crate::graphql::resolvers::mobile::contacts::resolvers::ContactsQuery
    );
    group!("Query", "sms", resolvers::sms::SmsQuery);
    group!(
        "Query",
        "calls",
        crate::graphql::resolvers::mobile::phone_calls::resolvers::CallsQuery
    );
    group!(
        "Query",
        "bookmarks",
        resolvers::bookmarks::bookmark::BookmarkQuery,
        resolvers::bookmarks::metadata::PageMetadataQuery
    );
    group!("Query", "notes", resolvers::notes::note::NoteQuery);
    group!("Query", "feeds", resolvers::feeds::feed::FeedQuery);
    group!(
        "Query",
        "notifications",
        resolvers::notifications::public::NotificationsQuery
    );
    group!(
        "Query",
        "clipboard",
        resolvers::clipboard::public::ClipboardQuery
    );
    group!(
        "Query",
        "screen_mirror",
        resolvers::screen_mirror::public::ScreenMirrorQuery
    );
    group!(
        "Query",
        "favorites",
        resolvers::favorites::favorite_folder::FavoriteFolderQuery
    );
    group!("Query", "docs", resolvers::docs::public::DocsQuery);
    group!("Query", "database", resolvers::database::public::DbQuery);
    group!(
        "Mutation",
        "app",
        resolvers::app::cache::CacheMutation,
        resolvers::app::platform::PlatformMutation
    );
    group!(
        "Mutation",
        "settings",
        resolvers::settings::public::PrefsMutation
    );
    group!(
        "Mutation",
        "casting",
        resolvers::casting::sender::DlnaSenderMutation
    );
    group!("Mutation", "peers", resolvers::peers::public::PeersMutation);
    group!(
        "Mutation",
        "shares",
        resolvers::shares::queue::SharedDownloadMutation,
        resolvers::shares::client::SharedClientMutation,
        resolvers::shares::host::ShareHostMutation
    );
    group!(
        "Mutation",
        "downloads",
        resolvers::downloads::queue::ChatDownloadMutation,
        resolvers::downloads::public::DownloadMutation
    );
    group!(
        "Mutation",
        "discovery",
        resolvers::discovery::mdns::MdnsMutation
    );
    group!(
        "Mutation",
        "files",
        resolvers::files::trash::TrashMutation,
        resolvers::files::tasks::FileTaskMutation,
        resolvers::files::app_files::AppFilesMutation,
        resolvers::files::operations::FileOpsMutation,
        resolvers::files::operations::MediaItemsMutation
    );
    group!(
        "Mutation",
        "images",
        resolvers::images::image_editor_project::ImageEditorProjectMutation,
        resolvers::images::public_index::ImageIndexMutation
    );
    group!(
        "Mutation",
        "audio",
        resolvers::audio::commands::AudioCommandsMutation,
        resolvers::audio::public::AudioMutation
    );
    group!(
        "Mutation",
        "chat",
        resolvers::chat::host::ChatHostMutation,
        resolvers::chat::public::ChatMutation
    );
    group!("Mutation", "tags", resolvers::tags::public::TagsMutation);
    group!(
        "Mutation",
        "media",
        resolvers::media::scan::MediaScanMutation,
        resolvers::media::play_history::MediaPlayHistoryMutation
    );
    group!(
        "Mutation",
        "clipboard",
        resolvers::clipboard::public::ClipboardMutation
    );
    group!(
        "Mutation",
        "bookmarks",
        resolvers::bookmarks::bookmark::BookmarkMutation
    );
    group!(
        "Mutation",
        "pomodoro",
        resolvers::pomodoro::state::PomodoroControlMutation,
        resolvers::pomodoro::pomodoro::PomodoroMutation
    );
    group!(
        "Mutation",
        "device",
        resolvers::device::sessions::SessionsMutation,
        resolvers::device::public::DeviceMutation
    );
    group!(
        "Mutation",
        "packages",
        resolvers::packages::public::PackagesMutation
    );
    group!(
        "Mutation",
        "contacts",
        crate::graphql::resolvers::mobile::contacts::resolvers::ContactsMutation
    );
    group!("Mutation", "sms", resolvers::sms::SmsMutation);
    group!(
        "Mutation",
        "calls",
        crate::graphql::resolvers::mobile::phone_calls::resolvers::CallsMutation
    );
    group!("Mutation", "notes", resolvers::notes::note::NoteMutation);
    group!("Mutation", "feeds", resolvers::feeds::feed::FeedMutation);
    group!(
        "Mutation",
        "notifications",
        resolvers::notifications::public::NotificationsMutation
    );
    group!(
        "Mutation",
        "screen_mirror",
        resolvers::screen_mirror::public::ScreenMirrorMutation,
        resolvers::screen_mirror::public::SettingsMutation
    );
    group!(
        "Mutation",
        "favorites",
        resolvers::favorites::favorite_folder::FavoriteFolderMutation
    );
    group!(
        "Mutation",
        "database",
        resolvers::database::public::DbMutation
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
        if matches!(name, "Query" | "Mutation") && declaration.starts_with("type ") {
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
                    roots
                        .entry((group, name.into()))
                        .or_default()
                        .push((field, pending.join("\n")));
                    pending.clear();
                }
            }
            assert!(pending.iter().all(|line| line.trim().is_empty()));
        } else {
            let group = types
                .get(name)
                .filter(|groups| groups.len() == 1)
                .and_then(|groups| groups.first())
                .map(String::as_str)
                .unwrap_or("common");
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
        let prefix = if defined.insert(root.clone()) {
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

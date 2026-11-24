use super::*;
use plain_libs::rusqlite::params;
use plain_libs::serde_json::json;

const TIME: &str = "2025-04-03T07:08:09Z";
const LOCAL: &str = "706c6169-0001-8000-8000-000000003039";
const PEER: &str = "706c6169-0001-8001-bfff-ffffffffffff";

#[test]
fn imports_an_older_room_database_with_channel_and_pairing_references() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let old = temp.path().join("old");
    let current = temp.path().join("current");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&current).unwrap();
    let storage = LegacyStorage {
        database_directory: old.clone(),
        preferences_directory: old.clone(),
        files_directory: old,
    };
    let source = Connection::open(storage.database()).unwrap();
    source.execute_batch(include_str!("room-v8.sql")).unwrap();
    let pairing_key = base64_encode(&[0x43; 32]);
    source.execute("INSERT INTO peers(id,name,ip,key,public_key,status,port,device_type,created_at,updated_at) VALUES ('9ix','Paired fixture','',?1,'','paired',0,'phone',?2,?2)", params![pairing_key,TIME]).unwrap();
    source.execute("INSERT INTO chat_groups(id,name,key,members,created_at,updated_at) VALUES ('ab','Group fixture',?1,'[\"9ix\"]',?2,?2)",params![pairing_key,TIME]).unwrap();
    source.execute("INSERT INTO chats(id,from_id,to_id,group_id,status,content,created_at,updated_at) VALUES ('ac','9ix','','ab','sent','{\"type\":\"text\",\"value\":{\"text\":\"Older fixture\"}}',?1,?1)",[TIME]).unwrap();
    source.execute("INSERT INTO feeds(id,name,url,fetch_content,created_at,updated_at) VALUES ('ad','Old feed','https://example.com/rss',0,?1,?1)",[TIME]).unwrap();
    let attachment_path = storage.files_directory.join("legacy-text.txt");
    let attachment_bytes = b"old chat text";
    std::fs::write(&attachment_path, attachment_bytes).unwrap();
    let attachment = json!({"type":"files","value":{"items":[{"id":"af","uri":attachment_path,
        "fileName":"legacy-text.txt","size":attachment_bytes.len()}]}});
    source.execute("INSERT INTO chats(id,from_id,to_id,group_id,status,content,created_at,updated_at) VALUES ('ae','me','9ix','','sent',?1,?2,?2)",params![attachment.to_string(), TIME]).unwrap();
    let current_db = Db::open(&current.join("plain.db")).unwrap();
    assert_eq!(
        storage
            .import(&current_db, &Prefs::in_memory(), &current)
            .unwrap(),
        Status::Imported
    );
    let message = current_db
        .get_chat_by_id(&short_id::convert("ac").unwrap())
        .unwrap();
    assert_eq!(message.from_id, LOCAL);
    assert_eq!(
        message.channel_id.unwrap(),
        short_id::convert("ab").unwrap()
    );
    let message = current_db
        .get_chat_by_id(&short_id::convert("ae").unwrap())
        .unwrap();
    let content: Value = serde_json::from_str(&message.content).unwrap();
    assert_eq!(
        content["value"]["items"][0]["attachment"]["id"],
        short_id::convert("af").unwrap()
    );
    let fid = content["value"]["items"][0]["attachment"]["uri"]
        .as_str()
        .unwrap()
        .strip_prefix("fid:")
        .unwrap();
    current_db.with_conn(|db| {
        assert_eq!(
            db.query_row("SELECT key FROM peers WHERE id=?1", [LOCAL], |r| r
                .get::<_, [u8; 32]>(0))
                .unwrap(),
            [0x43; 32]
        );
        let members: String = db
            .query_row("SELECT members FROM chat_channels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&members).unwrap()[0]["peerId"],
            LOCAL
        );
        assert_eq!(
            db.query_row("SELECT id FROM feeds", [], |r| r.get::<_, String>(0))
                .unwrap(),
            short_id::convert("ad").unwrap()
        );
        let path: String = db
            .query_row("SELECT real_path FROM app_files", [], |row| row.get(0))
            .unwrap();
        assert!(path.ends_with(fid));
        assert_eq!(std::fs::read(current.join(path)).unwrap(), attachment_bytes);
    });
    assert_eq!(
        storage.clear(&current_db, &current).unwrap(),
        Status::Cleared
    );
    assert!(!attachment_path.exists());
    assert_eq!(
        source
            .query_row("SELECT count(*) FROM chat_groups", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

fn fixture(root: &Path) -> (LegacyStorage, PathBuf, String) {
    let old = root.join("old");
    let current = root.join("current");
    std::fs::create_dir_all(old.join("datastore")).unwrap();
    std::fs::create_dir(&current).unwrap();
    let storage = LegacyStorage {
        database_directory: old.clone(),
        preferences_directory: old.clone(),
        files_directory: old,
    };
    let db = Connection::open(storage.database()).unwrap();
    db.execute_batch(include_str!("room-v25.sql")).unwrap();
    let key = base64_encode(&[0x45; 32]);
    db.execute(
        "INSERT INTO shares VALUES ('share','Test share','',?1,NULL,1,'[]',?2,?2)",
        params![base64_encode(&[0x46; 32]), TIME],
    ).unwrap();
    db.execute("INSERT INTO peers VALUES ('1y2p0ij32e8e7','Test peer','',?1,'','PAIRED',8443,'PHONE',?2,?2)", params![key, TIME]).unwrap();
    db.execute("INSERT INTO chat_channels VALUES ('channel','Test channel',?1,'me','[{\"id\":\"9ix\"},{\"id\":\"1y2p0ij32e8e7\"}]',2,'JOINED',?2,?2)", params![key, TIME]).unwrap();
    db.execute(
        "INSERT INTO pomodoro_items VALUES ('timer','2025-04-03',2,3000,600,?1,?1)",
        [TIME],
    )
    .unwrap();
    db.execute(
        "INSERT INTO image_editor_projects VALUES ('preserve','test',NULL,16,16,1,?1,?1)",
        [TIME],
    )
    .unwrap();

    let contents = b"old contact attachment";
    let digest =
        plain_libs::utils::hex::bytes_to_hex(&<sha2::Sha256 as sha2::Digest>::digest(contents));
    let path = format!("{}/{}/{}.txt", &digest[..2], &digest[2..4], digest);
    let source = storage.files_directory.join(&path);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, contents).unwrap();
    db.execute(
        "INSERT INTO files VALUES (?1,?2,'text/plain',?3,1,?1,?4,?4)",
        params![digest, contents.len() as i64, path, TIME],
    )
    .unwrap();
    let content = json!({"type":"FILES","value":{"items":[{
        "id": "item", "uri": format!("fid:{digest}.txt"), "size": contents.len(), "duration": 12,
    }, {
        "id": "remote-item", "uri": "fsid:expired-test-reference", "size": 32, "fileName": "missing.txt",
    }]}});
    let delivery = json!({"results":[{"peerId":"1y2p0ij32e8e7","peerName":"Test peer"}]});
    db.execute(
        "INSERT INTO chats VALUES ('message','me','','channel','SENT',?1,?2,?3,?3)",
        params![delivery.to_string(), content.to_string(), TIME],
    )
    .unwrap();

    let preferences = [
        ("client_id", "9ix"),
        ("scan_history", r#"["new","old"]"#),
        ("quick_note_draft", "old note draft"),
        ("chat_input_text", "old global chat draft"),
        ("locale", "en"),
    ]
    .into_iter()
    .flat_map(|(key, value)| {
        fn string(number: u8, value: &[u8]) -> Vec<u8> {
            assert!(value.len() < 128);
            [vec![(number << 3) | 2, value.len() as u8], value.to_vec()].concat()
        }
        string(
            1,
            &[
                string(1, key.as_bytes()),
                string(2, &string(5, value.as_bytes())),
            ]
            .concat(),
        )
    })
    .collect::<Vec<_>>();
    std::fs::write(storage.preferences(), preferences).unwrap();
    (storage, current, path)
}

#[test]
fn startup_migrates_existing_identity_and_renames_legacy_settings() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, _) = fixture(temp.path());
    let prefs_path = current.join("prefs.json");
    let prefs = Prefs::load(&prefs_path).unwrap();
    crate::features::prefs::security::bootstrap(&prefs).unwrap();
    let original = std::fs::read(storage.preferences()).unwrap();

    assert!(storage.prepare_identity(&prefs).unwrap());

    assert_eq!(
        Prefs::load(&prefs_path)
            .unwrap()
            .get::<String>(pref_keys::CLIENT_ID)
            .unwrap()
            .as_deref(),
        Some(LOCAL)
    );
    assert!(!storage.preferences().exists());
    assert_eq!(
        std::fs::read(storage.backup_preferences()).unwrap(),
        original
    );
    assert!(!storage.prepare_identity(&prefs).unwrap());
}

#[test]
fn startup_leaves_invalid_settings_available_for_retry() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, _, _) = fixture(temp.path());
    let prefs = Prefs::in_memory();
    std::fs::write(storage.preferences(), [0xff]).unwrap();

    assert!(storage.prepare_identity(&prefs).is_err());
    assert!(storage.preferences().is_file());
    assert!(!storage.backup_preferences().exists());
    assert!(prefs.get::<String>(pref_keys::CLIENT_ID).unwrap().is_none());
}

#[test]
fn deferred_import_reads_backup_and_deletes_only_the_imported_data() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, old_asset) = fixture(temp.path());
    let db = Db::open(&current.join("plain.db")).unwrap();
    let prefs = Prefs::in_memory();
    storage.prepare_identity(&prefs).unwrap();
    assert_eq!(
        prefs
            .get::<String>(pref_keys::CLIENT_ID)
            .unwrap()
            .as_deref(),
        Some(LOCAL)
    );
    assert_eq!(storage.status(&db, &current).unwrap(), Status::Pending);
    assert!(storage.clear(&db, &current).is_err());
    assert!(!storage.prepare_identity(&prefs).unwrap());
    assert!(db.with_conn(|conn| Receipt::read(conn)).unwrap().is_none());
    assert!(
        db.get_chat_by_id(&short_id::convert("message").unwrap())
            .is_none()
    );

    let status = storage.import(&db, &prefs, &current).unwrap();
    assert_eq!(status, Status::Imported);
    assert_eq!(
        storage.import(&db, &prefs, &current).unwrap(),
        Status::Imported
    );
    let message = db
        .get_chat_by_id(&short_id::convert("message").unwrap())
        .unwrap();
    assert_eq!(message.from_id, "me");
    let content: Value = serde_json::from_str(&message.content).unwrap();
    assert_eq!(content["type"], "MESSAGE");
    assert_eq!(content["value"]["items"][0]["attachment"]["durationMs"], 12);
    assert_eq!(content["value"]["items"].as_array().unwrap().len(), 1);
    assert!(content["value"]["text"].is_null());
    let deliveries: Value = serde_json::from_str(message.status_data.as_deref().unwrap()).unwrap();
    assert_eq!(deliveries["results"][0]["peerId"], PEER);
    db.with_conn(|conn| {
        let peer_key: [u8; 32] = conn
            .query_row("SELECT key FROM peers WHERE id=?1", [PEER], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(peer_key, [0x45; 32]);
        assert_eq!(
            conn.query_row("SELECT key FROM chat_channels", [], |row| row.get::<_, [u8; 32]>(0)).unwrap(),
            [0x45; 32]
        );
        assert_eq!(
            conn.query_row("SELECT url_token FROM shares", [], |row| row.get::<_, [u8; 32]>(0)).unwrap(),
            [0x46; 32]
        );
        let (owner, members): (String, String) = conn
            .query_row("SELECT owner_id,members FROM chat_channels", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(owner, LOCAL);
        let members: Value = serde_json::from_str(&members).unwrap();
        assert_eq!(members[0]["peerId"], LOCAL);
        assert_eq!(members[1]["peerId"], PEER);
        assert_eq!(
            conn.query_row("SELECT completed_count FROM pomodoro_items", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT text FROM scan_history ORDER BY rowid DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "new"
        );
        assert_eq!(
            conn.query_row("SELECT ref_count FROM app_files", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    });
    assert!(storage.files_directory.join(&old_asset).is_file());
    assert!(current.join("files").join(&old_asset).is_file());
    assert_eq!(storage.clear(&db, &current).unwrap(), Status::Cleared);
    assert_eq!(storage.clear(&db, &current).unwrap(), Status::Cleared);
    assert!(current.join("files").join(&old_asset).is_file());
    assert!(!storage.files_directory.join(&old_asset).exists());
    let retained = Connection::open(storage.database()).unwrap();
    assert_eq!(
        retained
            .query_row("SELECT count(*) FROM image_editor_projects", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        retained
            .query_row("SELECT count(*) FROM chats", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        preferences::read(&storage.backup_preferences())
            .unwrap()
            .client_id
            .is_none()
    );
    assert!(
        std::fs::read(storage.backup_preferences())
            .unwrap()
            .windows(6)
            .any(|bytes| bytes == b"locale")
    );
}

#[test]
fn an_attachment_only_message_survives_without_the_unavailable_file() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, _) = fixture(temp.path());
    let old = Connection::open(storage.database()).unwrap();
    let content = json!({"type":"FILES","value":{"items":[{
        "id": "remote-item", "uri": "fsid:expired-test-reference", "size": 32, "fileName": "missing.txt",
    }]}});
    old.execute("UPDATE chats SET content=?1", [content.to_string()])
        .unwrap();
    let db = Db::open(&current.join("plain.db")).unwrap();
    let prefs = Prefs::in_memory();
    storage.prepare_identity(&prefs).unwrap();

    storage.import(&db, &prefs, &current).unwrap();

    let message = db
        .get_chat_by_id(&short_id::convert("message").unwrap())
        .unwrap();
    let content: Value = serde_json::from_str(&message.content).unwrap();
    assert_eq!(content["type"], "MESSAGE");
    assert!(content["value"]["text"].is_null());
    assert_eq!(content["value"]["items"], json!([]));
}

#[test]
fn the_global_chat_draft_stays_in_the_old_preferences() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, _) = fixture(temp.path());
    let db = Db::open(&current.join("plain.db")).unwrap();
    let prefs = Prefs::in_memory();
    storage.prepare_identity(&prefs).unwrap();

    storage.import(&db, &prefs, &current).unwrap();
    assert!(
        db.key_values()
            .get("chat_input_text:peer:local")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        db.key_values()
            .get_json::<String>("quick_note_draft")
            .unwrap()
            .as_deref(),
        Some("old note draft")
    );

    storage.clear(&db, &current).unwrap();
    let old_settings = std::fs::read_to_string(storage.backup_preferences()).unwrap();
    assert!(old_settings.contains("old global chat draft"));
    assert!(!old_settings.contains("old note draft"));
}

#[test]
fn an_invalid_later_row_keeps_the_source_and_rolls_back_the_target() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, asset) = fixture(temp.path());
    let old = Connection::open(storage.database()).unwrap();
    old.execute("UPDATE chats SET content='invalid legacy chat JSON'", [])
        .unwrap();
    let db = Db::open(&current.join("plain.db")).unwrap();
    let prefs = Prefs::in_memory();
    storage.prepare_identity(&prefs).unwrap();

    assert!(storage.import(&db, &prefs, &current).is_err());
    assert_eq!(storage.status(&db, &current).unwrap(), Status::Pending);
    assert!(storage.files_directory.join(&asset).is_file());
    assert!(!current.join("files").join(&asset).exists());
    assert!(
        db.get_chat_by_id(&short_id::convert("message").unwrap())
            .is_none()
    );
    db.with_conn(|conn| {
        assert_eq!(
            conn.query_row("SELECT count(*) FROM peers", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM app_files", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    });
}

#[test]
fn an_install_without_legacy_storage_has_no_upgrade() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let target = temp.path().join("current");
    std::fs::create_dir(&target).unwrap();
    let db = Db::open(&target.join("plain.db")).unwrap();
    let legacy = temp.path().join("empty");
    let storage = LegacyStorage {
        database_directory: legacy.clone(),
        preferences_directory: legacy.clone(),
        files_directory: legacy,
    };
    assert_eq!(storage.status(&db, &target).unwrap(), Status::Unavailable);
    assert!(!storage.prepare_identity(&Prefs::in_memory()).unwrap());
}

#[test]
fn a_feed_added_before_import_keeps_its_entries_and_gains_legacy_tags() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let (storage, current, _) = fixture(temp.path());
    let old = Connection::open(storage.database()).unwrap();
    let url = "https://example.com/feed.xml";
    old.execute(
        "INSERT INTO feeds VALUES ('oldfeed','Older title',?1,0,NULL,'{}',?2,?2)",
        params![url, TIME],
    )
    .unwrap();
    old.execute("INSERT INTO feed_entries VALUES ('oldentry','Older entry','','','','','','oldfeed','guid',?1,0,?1,?1)", [TIME]).unwrap();
    old.execute(
        "INSERT INTO tags VALUES ('oldtag','Keep',7,1,?1,?1)",
        [TIME],
    )
    .unwrap();
    old.execute(
        "INSERT INTO tag_relations VALUES ('oldtag','oldentry',7,?1,0,'')",
        [TIME],
    )
    .unwrap();

    let db = Db::open(&current.join("plain.db")).unwrap();
    let prefs = Prefs::in_memory();
    storage.prepare_identity(&prefs).unwrap();
    db.with_conn(|conn| {
        conn.execute("INSERT INTO feeds(id,name,url,fetch_full_content,created_at,updated_at) VALUES ('00000000-0000-7000-8000-000000000101','Current title',?1,0,?2,?2)", params![url, TIME]).unwrap();
        conn.execute("INSERT INTO feed_entries VALUES ('00000000-0000-7000-8000-000000000102','Current entry','','','','','','00000000-0000-7000-8000-000000000101','guid',?1,1,?1,?1)", [TIME]).unwrap();
    });

    assert_eq!(
        storage.import(&db, &prefs, &current).unwrap(),
        Status::Imported
    );
    db.with_conn(|conn| {
        let tag_target: String = conn
            .query_row(
                "SELECT target_key FROM tag_relations WHERE tag_id=?1",
                [short_id::convert("oldtag").unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tag_target, "00000000-0000-7000-8000-000000000102");
        let title: String = conn
            .query_row("SELECT name FROM feeds WHERE url=?1", [url], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(title, "Current title");
        assert_eq!(
            conn.query_row("SELECT count(*) FROM feed_entries", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    });
}

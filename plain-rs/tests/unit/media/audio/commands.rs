use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;
struct Fake {
    commands: Vec<(Action, String, i64)>,
    fail: bool,
}
impl Engine for Fake {
    fn metadata(&mut self, path: &str) -> DomainResult<AudioTrack> {
        Ok(AudioTrack::from_path_stem(path))
    }
    fn execute(&mut self, c: &EngineCommand) -> DomainResult<EngineReport> {
        if self.fail {
            return Err(DomainError::Other("player unavailable".into()));
        }
        self.commands
            .push((c.action, c.playback.path.clone(), c.playback.position_ms));
        Ok(EngineReport {
            path: c.playback.path.clone(),
            position_ms: c.playback.position_ms,
            revision: c.playback.revision,
            loaded: c.new_track,
        })
    }
}
#[test]
fn commands_preserve_resume_and_repeat_one_only_applies_to_completion() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let mut engine = Fake {
        commands: vec![],
        fail: false,
    };
    fixtures::register_audio_tracks(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
    );
    play_path(&db, &prefs, &mut engine, "one").unwrap();
    audio_queue::enqueue(&db, &[AudioTrack::from_path_stem("two")], false).unwrap();
    command(&db, &prefs, &mut engine, Action::Seek, 3_000_000_123, 1.0).unwrap();
    command(&db, &prefs, &mut engine, Action::Pause, 0, 1.0).unwrap();
    let resumed = command(&db, &prefs, &mut engine, Action::Play, 0, 1.0).unwrap();
    assert_eq!(resumed.position_ms, 3_000_000_123);
    audio_queue::save_audio_mode(&prefs, "REPEAT_ONE").unwrap();
    let repeated = command(&db, &prefs, &mut engine, Action::Completed, 0, 1.0).unwrap();
    assert_eq!(repeated.path, "one");
    assert_eq!(repeated.position_ms, 0);
    let next = command(&db, &prefs, &mut engine, Action::Next, 0, 1.0).unwrap();
    assert_eq!(next.path, "two");
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    assert!(command(&db, &prefs, &mut engine, Action::SetSpeed, 0, f32::NAN).is_err());
    engine.fail = true;
    assert!(command(&db, &prefs, &mut engine, Action::Pause, 0, 1.0).is_err());
}

#[test]
fn restarting_the_core_plays_the_remembered_track_from_zero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let prefs = std::sync::Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let db = Db::open(&path).unwrap();
    let mut engine = Fake {
        commands: vec![],
        fail: false,
    };
    fixtures::register_audio_tracks(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
    );
    play_path(&db, &prefs, &mut engine, "one").unwrap();
    audio_queue::enqueue(&db, &[AudioTrack::from_path_stem("two")], false).unwrap();
    command(&db, &prefs, &mut engine, Action::Next, 0, 1.0).unwrap();
    command(&db, &prefs, &mut engine, Action::Seek, 123_456, 1.0).unwrap();
    command(&db, &prefs, &mut engine, Action::Pause, 0, 1.0).unwrap();
    assert_eq!(db.audio_current_file_id().unwrap(), "test:two");
    drop(db);
    let db = Db::open(&path).unwrap();
    fixtures::register_audio_tracks(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
    );
    audio_playback::invalidate(&db).unwrap();
    let restored = command(&db, &prefs, &mut engine, Action::Play, 0, 1.0).unwrap();
    assert_eq!(restored.path, "two");
    assert_eq!(restored.position_ms, 0);
    assert!(matches!(engine.commands.last().unwrap(), (Action::Play, path, 0) if path == "two"));
}

#[test]
fn path_playback_is_temporary_and_never_enqueues_or_fabricates_an_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let db = Db::open(&path).unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let mut engine = Fake {
        commands: vec![],
        fail: false,
    };
    let playback = play_path(&db, &prefs, &mut engine, "/unindexed.mp3").unwrap();
    assert_eq!(playback.path, "/unindexed.mp3");
    assert_eq!(db.audio_current_file_id(), None);
    assert_eq!(audio_queue::queue_total(&db).unwrap(), 0);
    let track = AudioTrack::from_path_stem("/unindexed.mp3");
    assert!(audio_playback::started(&db, &track, playback.revision).unwrap());
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    drop(db);
    let reopened = Db::open(&path).unwrap();
    assert_eq!(audio_queue::get_audio_current(&reopened).unwrap(), "");
}

#[test]
fn completion_checks_path_revision_and_duplicates_before_advancing_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    let mut engine = Fake {
        commands: vec![],
        fail: false,
    };
    fixtures::register_audio_tracks(
        &db,
        &[
            AudioTrack::from_path_stem("one"),
            AudioTrack::from_path_stem("two"),
        ],
    );
    let first = play_path(&db, &prefs, &mut engine, "one").unwrap();
    audio_queue::enqueue(&db, &[AudioTrack::from_path_stem("two")], false).unwrap();
    let seeked = command(&db, &prefs, &mut engine, Action::Seek, 4000, 1.0).unwrap();
    let calls = engine.commands.len();
    assert!(!completed(&db, &prefs, &mut engine, "one", first.revision).unwrap());
    assert!(!completed(&db, &prefs, &mut engine, "other", seeked.revision).unwrap());
    assert_eq!(engine.commands.len(), calls);
    assert_eq!(audio_playback::snapshot(&db).unwrap(), seeked);
    assert!(completed(&db, &prefs, &mut engine, "one", seeked.revision).unwrap());
    let next = audio_playback::snapshot(&db).unwrap();
    assert_eq!(next.path, "two");
    assert!(!completed(&db, &prefs, &mut engine, "one", seeked.revision).unwrap());
    assert_eq!(engine.commands.len(), calls + 1);
    audio_queue::save_audio_mode(&prefs, "REPEAT_ONE").unwrap();
    assert!(completed(&db, &prefs, &mut engine, "two", next.revision).unwrap());
    let repeated = audio_playback::snapshot(&db).unwrap();
    assert_eq!(repeated.path, "two");
    assert_ne!(repeated.revision, next.revision);
    assert!(!completed(&db, &prefs, &mut engine, "two", next.revision).unwrap());
    assert!(completed(&db, &prefs, &mut engine, "", repeated.revision).is_err());
    assert!(completed(&db, &prefs, &mut engine, "two", -1).is_err());
}

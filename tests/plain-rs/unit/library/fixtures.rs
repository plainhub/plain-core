#![allow(dead_code)]

use plain_rs::db::Db;
use plain_rs::error::DomainResult;
use plain_rs::media::audio::queue::AudioTrack;
pub fn test_db(tag: &str) -> Db {
    let dir = tempfile::Builder::new()
        .prefix(&format!("plain-rs-library-{tag}-"))
        .tempdir()
        .unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    plain_rs::test_tempdirs::retain(dir);
    db
}

pub fn audio(path: &str, title: &str, duration_ms: i64) -> AudioTrack {
    AudioTrack {
        file_id: None,
        title: title.to_string(),
        artist: "A".to_string(),
        path: path.to_string(),
        duration_ms,
        album_id: String::new(),
    }
}

pub fn paths_of(items: &[AudioTrack]) -> Vec<String> {
    items.iter().map(|a| a.path.clone()).collect()
}

pub fn register_audio(db: &Db, track: &AudioTrack) {
    if track.path.is_empty() || track.duration_ms < 0 {
        return;
    }
    db.with_conn(|conn| conn.execute("INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(file_id) DO UPDATE SET title=excluded.title,artist=excluded.artist,duration_ms=excluded.duration_ms,album_id=CASE WHEN excluded.album_id<>'' THEN excluded.album_id ELSE platform_audio_media.album_id END",rusqlite::params![format!("test:{}",track.path),track.path,track.title,track.artist,track.duration_ms,track.album_id])).unwrap();
}

pub fn register_audio_tracks(db: &Db, tracks: &[AudioTrack]) {
    for track in tracks {
        register_audio(db, track);
    }
}

pub fn register_native_audio(db: &Db, track: &AudioTrack) {
    db.record_media_files(&[plain_rs::db::MediaFileMetadata {
        data_type: plain_rs::enums::DataType::Audio,
        path: track.path.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: String::new(),
        album_artist: String::new(),
        bucket_id: String::new(),
        size_bytes: 1,
        duration_ms: track.duration_ms,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }])
    .unwrap();
}

pub fn indexed_enqueue(db: &Db, tracks: &[AudioTrack], next: bool) -> DomainResult<()> {
    register_audio_tracks(db, tracks);
    plain_rs::media::audio::queue::enqueue(db, tracks, next)
}

pub fn indexed_add_playlist_items(db: &Db, id: &str, tracks: &[AudioTrack]) -> DomainResult<usize> {
    register_audio_tracks(db, tracks);
    plain_rs::media::audio::queue::add_playlist_items(db, id, tracks)
}

pub fn indexed_on_playing(
    db: &Db,
    path: &str,
    title: &str,
    artist: &str,
    duration_ms: i64,
) -> DomainResult<()> {
    register_audio(
        db,
        &AudioTrack {
            file_id: None,
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            duration_ms,
            album_id: String::new(),
        },
    );
    plain_rs::media::audio::queue::on_playing(db, path)
}

pub fn indexed_current(db: &Db, path: &str) -> DomainResult<()> {
    if !path.is_empty() {
        register_audio(db, &AudioTrack::from_path_stem(path));
    }
    plain_rs::media::audio::queue::save_audio_current(db, path)
}

pub fn history_row(db: &Db, file_id: &str) -> Option<plain_rs::db::MediaPlayHistoryRow> {
    plain_rs::media::play_history::matching(
        db,
        &[plain_rs::utils::search_dsl::FilterField {
            name: "ids".into(),
            op: String::new(),
            value: file_id.into(),
        }],
    )
    .unwrap()
    .into_iter()
    .next()
}

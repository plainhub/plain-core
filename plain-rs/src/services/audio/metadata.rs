#[cfg(test)]
use plain_rs::db::{MediaFileRow, audio_catalog::AudioMedia};
use plain_rs::error::{DomainError, DomainResult};
use plain_rs::media::audio::queue::AudioTrack;

pub async fn read_uri(
    resources: &plain_rs::host::resources::NativeResources,
    cache: &std::path::Path,
    path: &str,
) -> DomainResult<AudioTrack> {
    let source = path.to_owned();
    plain_rs::services::files::input::read_source(resources, cache, path, move |reader| {
        Ok(track(
            &source,
            plain_rs::media::probe::probe_audio_reader(reader),
        ))
    })
    .await
    .map_err(DomainError::Other)
}

fn track(path: &str, facts: plain_rs::media::probe::ProbedMeta) -> AudioTrack {
    AudioTrack {
        file_id: None,
        path: path.to_owned(),
        title: if facts.title.is_empty() {
            AudioTrack::from_path_stem(path).title
        } else {
            facts.title
        },
        album_id: plain_rs::media::audio::metadata::album_id(
            &facts.album,
            &facts.album_artist,
            &facts.artist,
        ),
        artist: plain_rs::media::audio::metadata::normalize_artist(&facts.artist),
        duration_ms: facts.duration_ms,
    }
}

#[cfg(test)]
pub fn from_row(row: MediaFileRow) -> AudioMedia {
    AudioMedia {
        file_id: Some(row.id),
        aliases: Vec::new(),
        track: AudioTrack {
            file_id: None,
            path: row.path.clone(),
            title: if row.title.is_empty() {
                AudioTrack::from_path_stem(&row.path).title
            } else {
                row.title
            },
            artist: plain_rs::media::audio::metadata::normalize_artist(&row.artist),
            duration_ms: row.duration_ms,
            album_id: row.album_id,
        },
    }
}

pub fn read_file(path: &str) -> DomainResult<AudioTrack> {
    let local = plain_rs::utils::file_uri::local_path(path).map_err(DomainError::Other)?;
    if !std::fs::metadata(&local)
        .map_err(|error| DomainError::Other(error.to_string()))?
        .is_file()
    {
        return Err(DomainError::Other(
            "Audio source is not a regular file".into(),
        ));
    }
    let mut facts = plain_rs::media::probe::probe_media(&local, "audio");
    if facts.title.is_empty() {
        facts.title = AudioTrack::from_path_stem(&local).title;
    }
    Ok(track(path, facts))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/audio/metadata.rs"]
mod tests;

use crate::features::media::audio::queue::AudioTrack;
#[cfg(test)]
use crate::libs::db::{MediaFileRow, audio_catalog::AudioMedia};
use crate::libs::error::{DomainError, DomainResult};

pub async fn read_uri(
    resources: &crate::libs::host::resources::NativeResources,
    cache: &std::path::Path,
    path: &str,
) -> DomainResult<AudioTrack> {
    let source = path.to_owned();
    crate::features::services::files::input::read_source(resources, cache, path, move |reader| {
        Ok(track(
            &source,
            crate::libs::media::probe::probe_audio_reader(reader),
        ))
    })
    .await
    .map_err(DomainError::Other)
}

fn track(path: &str, facts: crate::libs::media::probe::ProbedMeta) -> AudioTrack {
    AudioTrack {
        file_id: None,
        path: path.to_owned(),
        title: if facts.title.is_empty() {
            AudioTrack::from_path_stem(path).title
        } else {
            facts.title
        },
        album_id: crate::features::media::audio::metadata::album_id(
            &facts.album,
            &facts.album_artist,
            &facts.artist,
        ),
        artist: crate::features::media::audio::metadata::normalize_artist(&facts.artist),
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
            artist: crate::features::media::audio::metadata::normalize_artist(&row.artist),
            duration_ms: row.duration_ms,
            album_id: row.album_id,
        },
    }
}

pub fn read_file(path: &str) -> DomainResult<AudioTrack> {
    let local = crate::libs::utils::file_uri::local_path(path).map_err(DomainError::Other)?;
    if !std::fs::metadata(&local)
        .map_err(|error| DomainError::Other(error.to_string()))?
        .is_file()
    {
        return Err(DomainError::Other(
            "Audio source is not a regular file".into(),
        ));
    }
    let mut facts = crate::libs::media::probe::probe_media(&local, "audio");
    if facts.title.is_empty() {
        facts.title = AudioTrack::from_path_stem(&local).title;
    }
    Ok(track(path, facts))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/audio/metadata.rs"]
mod tests;

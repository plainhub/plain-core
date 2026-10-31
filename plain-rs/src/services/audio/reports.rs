use plain_rs::host::audio::AudioHostReport;
use plain_rs::media::audio::{playback, queue::AudioTrack};
use plain_rs::runtime::Runtime as CoreRuntime;
pub async fn accept(
    state: &CoreRuntime,
    generation: u64,
    kind: &str,
    payload: &str,
) -> Result<bool, String> {
    let Some(report) = AudioHostReport::decode(kind, payload)? else {
        return Ok(false);
    };
    let host = state.host.clone();
    let audio = &state.audio;
    match report {
        AudioHostReport::Progress(report) => {
            audio
                .run_playback(move |db, _| {
                    if !host.is_current(generation) {
                        return Err(plain_rs::error::DomainError::Other(
                            "host disconnected".into(),
                        ));
                    }
                    playback::report(db, &report.path, report.revision, report.position_ms)
                })
                .await
                .map_err(|error| error.to_string())?;
        }
        AudioHostReport::Started(report) => {
            let accepted = audio
                .run_playback(move |db, _| {
                    if !host.is_current(generation) {
                        return Err(plain_rs::error::DomainError::Other(
                            "host disconnected".into(),
                        ));
                    }
                    let track = db
                        .with_conn(|connection| {
                            plain_rs::media::audio::queue::track_by_path_conn(
                                connection,
                                &report.path,
                            )
                        })?
                        .unwrap_or_else(|| AudioTrack::from_path_stem(&report.path));
                    let indexed = track.file_id.is_some();
                    Ok(playback::started(db, &track, report.revision)? && indexed)
                })
                .await
                .map_err(|error| error.to_string())?;
            if accepted {
                publish_library_updated(&state.host);
            }
        }
        AudioHostReport::Completed(report) => {
            if report.path.is_empty() || report.revision < 0 {
                return Err("invalid audio completion".into());
            }
            let prefs = state.prefs.clone();
            let accepted = audio
                .run_playback(move |db, engine| {
                    if !host.is_current(generation) {
                        return Err(plain_rs::error::DomainError::Other(
                            "host disconnected".into(),
                        ));
                    }
                    let current = playback::snapshot(db)?;
                    if current.path != report.path || current.revision != report.revision {
                        return Ok(false);
                    }
                    engine.refresh()?;
                    plain_rs::media::audio::commands::completed(
                        db,
                        &prefs,
                        engine,
                        &report.path,
                        report.revision,
                    )
                })
                .await
                .map_err(|error| error.to_string())?;
            if accepted {
                let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
                    "CONTENT_CHANGED",
                    "{}".into(),
                ));
            }
        }
    }
    Ok(true)
}

pub fn publish_library_updated(host: &std::sync::Arc<plain_rs::host::bridge::Host>) {
    host.invalidate("audioLibraryRefresh");
}

#[cfg(test)]
#[path = "../../../tests/unit/services/audio/reports.rs"]
mod tests;

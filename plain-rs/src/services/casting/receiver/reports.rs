use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::casting::receiver::types::DlnaPlaybackState;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Position {
    media_uri: String,
    position_ms: i64,
    duration_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Playback {
    media_uri: String,
    state: DlnaPlaybackState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Seek {
    media_uri: String,
    position_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stopped {
    media_uri: String,
}

enum Report {
    Position(Position),
    Playback(Playback),
    Seek(Seek),
    Stopped(Stopped),
}

impl Report {
    fn media_uri(&self) -> &str {
        match self {
            Self::Position(value) => &value.media_uri,
            Self::Playback(value) => &value.media_uri,
            Self::Seek(value) => &value.media_uri,
            Self::Stopped(value) => &value.media_uri,
        }
    }
}

pub async fn accept(
    state: &CoreRuntime,
    generation: u64,
    kind: &str,
    payload: &str,
) -> Result<bool, String> {
    let report = match kind {
        "DLNA_POSITION_REPORTED" => serde_json::from_str(payload).map(Report::Position),
        "DLNA_PLAYBACK_REPORTED" => serde_json::from_str(payload).map(Report::Playback),
        "DLNA_SEEK_CONSUMED_REPORTED" => serde_json::from_str(payload).map(Report::Seek),
        "DLNA_STOPPED_REPORTED" => serde_json::from_str(payload).map(Report::Stopped),
        _ => return Ok(false),
    }
    .map_err(|error| error.to_string())?;
    if *state.stop.borrow() {
        return Err("Core stopped".into());
    }
    if report.media_uri().is_empty() {
        return Err("media URI required".into());
    }
    let notify = !matches!(&report, Report::Position(_));
    state
        .dlna
        .apply_report(notify, |renderer| {
            if *state.stop.borrow()
                || !state.host.is_current(generation)
                || renderer.media_uri != report.media_uri()
            {
                return false;
            }
            match report {
                Report::Position(value) => {
                    renderer.current_position_ms = value.position_ms.max(0);
                    renderer.duration_ms = value.duration_ms.max(0);
                }
                Report::Playback(value) => renderer.playback_state = value.state,
                Report::Seek(value) => {
                    if renderer.seek_target_ms != Some(value.position_ms) {
                        return false;
                    }
                    renderer.seek_target_ms = None;
                }
                Report::Stopped(_) => {
                    renderer.media_uri.clear();
                    renderer.media_title.clear();
                    renderer.media_album_art_uri.clear();
                    renderer.media_type =
                        plain_rs::services::casting::receiver::types::DlnaMediaType::UNKNOWN;
                    renderer.playback_state = DlnaPlaybackState::NoMediaPresent;
                    renderer.seek_target_ms = None;
                }
            }
            true
        })
        .await;
    Ok(true)
}

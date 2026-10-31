use crate::db::Db;
use crate::error::DomainError;
use crate::error::DomainResult;
use crate::media::audio::playback as audio_playback;
use crate::media::audio::playback::Playback;
use crate::media::audio::queue as audio_queue;
use crate::media::audio::queue::AudioTrack;
use crate::prefs::Prefs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[cfg_attr(feature = "graphql", graphql(name = "AudioCommandAction"))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Play,
    Pause,
    Seek,
    Next,
    Previous,
    Completed,
    Clear,
    Restart,
    SetSpeed,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCommand {
    pub action: Action,
    pub new_track: bool,
    pub track: Option<AudioTrack>,
    pub playback: Playback,
    pub speed: f32,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineReport {
    pub path: String,
    pub position_ms: i64,
    pub revision: i64,
    pub loaded: bool,
}
pub trait Engine {
    fn metadata(&mut self, path: &str) -> DomainResult<AudioTrack>;
    fn execute(&mut self, command: &EngineCommand) -> DomainResult<EngineReport>;
}
pub fn play_track(
    db: &Db,
    prefs: &Prefs,
    engine: &mut impl Engine,
    track: AudioTrack,
) -> DomainResult<Playback> {
    if track.path.trim().is_empty() || track.duration_ms < 0 {
        return Err(DomainError::Other("invalid audio track".into()));
    }
    let playback = audio_playback::prepare_track(db, &track)?;
    dispatch(
        db,
        prefs,
        engine,
        Action::Play,
        Some(track),
        playback,
        None,
        true,
    )
}
pub fn play_path(
    db: &Db,
    prefs: &Prefs,
    engine: &mut impl Engine,
    path: &str,
) -> DomainResult<Playback> {
    if path.trim().is_empty() {
        return Err(DomainError::Other("empty audio path".into()));
    }
    let track = engine.metadata(path)?;
    if track.path != path {
        return Err(DomainError::Other("audio metadata path mismatch".into()));
    }
    play_track(db, prefs, engine, track)
}
pub fn command(
    db: &Db,
    prefs: &Prefs,
    engine: &mut impl Engine,
    action: Action,
    position_ms: i64,
    speed: f32,
) -> DomainResult<Playback> {
    let mut actual = action;
    let mut track = None;
    let playback = match action {
        Action::Next | Action::Previous | Action::Completed => {
            let selection = match action {
                Action::Next => super::selection::AudioSelectionAction::Next,
                Action::Previous => super::selection::AudioSelectionAction::Previous,
                Action::Completed => super::selection::AudioSelectionAction::Completed,
                _ => unreachable!(),
            };
            let selected =
                super::selection::select(db, prefs, selection, |path| engine.metadata(path))?;
            if let Some(selected) = selected {
                track = Some(selected);
                actual = Action::Play;
                audio_playback::seek(db, 0)?
            } else {
                actual = Action::Pause;
                audio_playback::invalidate(db)?
            }
        }
        Action::Seek => audio_playback::seek(db, position_ms)?,
        Action::Clear => {
            audio_queue::save_audio_current(db, "")?;
            audio_playback::invalidate(db)?
        }
        Action::SetSpeed => {
            if !speed.is_finite() || speed <= 0.0 {
                return Err(DomainError::Other("invalid playback speed".into()));
            }
            prefs
                .set("audio_playback_speed", speed)
                .map_err(|e| DomainError::Other(e.to_string()))?;
            audio_playback::invalidate(db)?
        }
        _ => audio_playback::invalidate(db)?,
    };
    let new_track = track.is_some();
    if matches!(actual, Action::Play | Action::Seek) && track.is_none() {
        if playback.path.is_empty() {
            return Err(DomainError::Other("no selected audio".into()));
        }
        track = Some(engine.metadata(&playback.path)?);
    }
    dispatch(
        db,
        prefs,
        engine,
        actual,
        track,
        playback,
        matches!(action, Action::SetSpeed).then_some(speed),
        new_track,
    )
}
pub fn completed(
    db: &Db,
    prefs: &Prefs,
    engine: &mut impl Engine,
    path: &str,
    revision: i64,
) -> DomainResult<bool> {
    if path.is_empty() || revision < 0 {
        return Err(DomainError::Other("invalid audio completion".into()));
    }
    let current = audio_playback::snapshot(db)?;
    if current.path != path || current.revision != revision {
        return Ok(false);
    }
    command(db, prefs, engine, Action::Completed, 0, 1.0)?;
    Ok(true)
}

pub fn clear_engine(db: &Db, prefs: &Prefs, engine: &mut impl Engine) -> DomainResult<Playback> {
    let playback = audio_playback::invalidate(db)?;
    dispatch(
        db,
        prefs,
        engine,
        Action::Clear,
        None,
        playback,
        None,
        false,
    )
}

fn dispatch(
    db: &Db,
    prefs: &Prefs,
    engine: &mut impl Engine,
    action: Action,
    track: Option<AudioTrack>,
    playback: Playback,
    speed: Option<f32>,
    new_track: bool,
) -> DomainResult<Playback> {
    let speed = match speed {
        Some(value) => value,
        None => prefs
            .get::<f32>("audio_playback_speed")
            .map_err(|e| DomainError::Other(e.to_string()))?
            .unwrap_or(1.0),
    };
    if !speed.is_finite() || speed <= 0.0 {
        return Err(DomainError::Other("invalid stored playback speed".into()));
    }
    if track
        .as_ref()
        .is_some_and(|t| t.path != playback.path || t.duration_ms < 0)
    {
        return Err(DomainError::Other("invalid engine track".into()));
    }
    let revision = playback.revision;
    let selected_path = playback.path.clone();
    let report = engine.execute(&EngineCommand {
        action,
        new_track,
        track,
        playback,
        speed,
    })?;
    if report.revision != revision
        || report.position_ms < 0
        || (matches!(action, Action::Play | Action::Seek) && report.path != selected_path)
    {
        return Err(DomainError::Other(
            "invalid audio engine acknowledgement".into(),
        ));
    }
    if report.loaded {
        audio_playback::loaded(db, &report.path, revision)?;
    }
    if !matches!(action, Action::Clear) && !report.path.is_empty() {
        audio_playback::report(db, &report.path, revision, report.position_ms)?;
    }
    audio_playback::snapshot(db)
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/media/audio/commands.rs"]
mod tests;

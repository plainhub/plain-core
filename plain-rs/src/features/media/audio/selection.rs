use super::{
    play_mode::MediaPlayMode,
    playback::{self, Playback},
    queue::{self, AudioTrack},
};
use crate::{
    features::prefs::Prefs,
    libs::{
        db::Db,
        error::{DomainError, DomainResult},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
pub enum AudioSelectionAction {
    Next,
    Previous,
    Completed,
}

pub struct Selection {
    pub track: Option<AudioTrack>,
    pub playback: Playback,
}

pub fn select(
    db: &Db,
    prefs: &Prefs,
    action: AudioSelectionAction,
    mut metadata: impl FnMut(&str) -> DomainResult<AudioTrack>,
) -> DomainResult<Option<AudioTrack>> {
    let mode = queue::play_mode(prefs)?;
    if action == AudioSelectionAction::Completed && mode == MediaPlayMode::RepeatOne {
        let path = queue::get_audio_current(db)?;
        if path.is_empty() {
            return Ok(None);
        }
        return metadata(&path).map(Some);
    }
    queue::select_next(
        db,
        action != AudioSelectionAction::Previous,
        mode == MediaPlayMode::Shuffle,
    )
}

pub fn advance(
    db: &Db,
    prefs: &Prefs,
    action: AudioSelectionAction,
    expected: &Playback,
    metadata: impl FnMut(&str) -> DomainResult<AudioTrack>,
) -> DomainResult<Option<Selection>> {
    advance_with_shuffle(db, prefs, action, expected, false, metadata)
}

pub fn advance_with_shuffle(
    db: &Db,
    prefs: &Prefs,
    action: AudioSelectionAction,
    expected: &Playback,
    shuffle: bool,
    metadata: impl FnMut(&str) -> DomainResult<AudioTrack>,
) -> DomainResult<Option<Selection>> {
    if expected.revision < 0 {
        return Err(DomainError::Other(
            "invalid audio selection revision".into(),
        ));
    }
    let current = playback::snapshot(db)?;
    if current.path != expected.path || current.revision != expected.revision {
        return Ok(None);
    }
    let track = if shuffle && action != AudioSelectionAction::Completed {
        queue::select_next(db, action != AudioSelectionAction::Previous, true)?
    } else {
        select(db, prefs, action, metadata)?
    };
    let playback = if let Some(track) = &track {
        playback::prepare_track(db, track)?
    } else {
        playback::invalidate(db)?
    };
    Ok(Some(Selection { track, playback }))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/audio/selection.rs"]
mod tests;

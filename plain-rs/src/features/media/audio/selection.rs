use super::{
    play_mode::MediaPlayMode,
    queue::{self, AudioTrack},
};
use crate::features::prefs::Prefs;
use crate::infrastructure::db::Db;
use crate::infrastructure::error::DomainResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSelectionAction {
    Next,
    Previous,
    Completed,
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

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/audio/selection.rs"]
mod tests;

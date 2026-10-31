use plain_rs::host::bridge::Host;
use serde::{Deserialize, Serialize};

pub const PLAYBACK_STATE: &str = "systemAudioPlaybackState";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioPlaybackFacts {
    pub is_playing: bool,
    pub position_ms: i64,
}

impl Host {
    pub async fn audio_playback_facts(&self) -> Result<AudioPlaybackFacts, String> {
        let value = self.call(PLAYBACK_STATE, serde_json::json!({})).await?;
        serde_json::from_value(value)
            .map_err(|error| format!("Invalid audio playback facts: {error}"))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioProgressReport {
    pub path: String,
    pub revision: i64,
    pub position_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioPlaybackReport {
    pub path: String,
    pub revision: i64,
}

pub enum AudioHostReport {
    Progress(AudioProgressReport),
    Started(AudioPlaybackReport),
    Completed(AudioPlaybackReport),
}

impl AudioHostReport {
    pub fn decode(kind: &str, payload: &str) -> Result<Option<Self>, String> {
        let report = match kind {
            "AUDIO_PROGRESS_REPORTED" => serde_json::from_str(payload).map(Self::Progress),
            "AUDIO_STARTED_REPORTED" => serde_json::from_str(payload).map(Self::Started),
            "AUDIO_COMPLETED_REPORTED" => serde_json::from_str(payload).map(Self::Completed),
            _ => return Ok(None),
        };
        report.map(Some).map_err(|error| error.to_string())
    }
}

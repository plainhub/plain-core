use crate::server::ServerState;
use plain_rs::host::audio::{AudioPlaybackFacts, PLAYBACK_STATE};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct NativeAudio {
    state: ServerState,
}

impl NativeAudio {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn handle(&self, method: &str, params: Value) -> Result<Value, String> {
        self.state.native_client_id()?;
        let method = method.to_owned();
        tokio::task::spawn_blocking(move || match method.as_str() {
            PLAYBACK_STATE => serde_json::to_value(AudioPlaybackFacts {
                is_playing: false,
                position_ms: 0,
            })
            .map_err(|error| error.to_string()),
            "audioEngineCommand" => command(params),
            _ => Err("Unsupported audio host method".into()),
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

fn command(params: Value) -> Result<Value, String> {
    if params["action"] != "CLEAR" {
        return Err("Native audio playback is unavailable".into());
    }
    let mut playback = params["playback"].clone();
    if !playback.is_object() {
        return Err("Audio playback required".into());
    }
    playback["loaded"] = json!(false);
    Ok(playback)
}

pub fn handles(method: &str) -> bool {
    matches!(method, PLAYBACK_STATE | "audioEngineCommand")
}

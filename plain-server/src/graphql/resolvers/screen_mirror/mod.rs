//! Public `/graphql` screen-mirror roots.
//!

pub mod mutation;
pub mod query;

use crate::graphql::types::facts::{integer, text};
use plain_graphql::{Context, Enum, SimpleObject};
use plain_libs::serde_json::Value;
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum ScreenMirrorMode {
    #[default]
    Hd,
    Smooth,
}

impl ScreenMirrorMode {
    fn parse(value: &str) -> Self {
        match value {
            "SMOOTH" => Self::Smooth,
            _ => Self::Hd,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::Smooth => "SMOOTH",
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ScreenMirrorQuality {
    pub mode: ScreenMirrorMode,
    pub resolution: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ScreenMirrorVideoCodec {
    /// H.264 Annex-B codec configuration (SPS/PPS), base64-encoded.
    #[graphql(name = "annexB")]
    pub annex_b: String,
    /// Latest H.264 keyframe, base64-encoded; null until the pipeline emits one.
    #[graphql(name = "keyFrame")]
    pub key_frame: Option<String>,
}

async fn state(ctx: &Context<'_>) -> plain_graphql::Result<Value> {
    plain_rs::features::services::capabilities::require(
        ctx.data_unchecked::<Arc<Host>>(),
        Capability::ScreenMirror,
    )
    .await
    .map_err(|error| plain_graphql::Error::from(error))?;
    ctx.data_unchecked::<Arc<Host>>()
        .call("systemScreenMirrorState", plain_libs::serde_json::json!({}))
        .await
        .map_err(|error| plain_graphql::Error::new(error))
}

fn codec(value: &Value) -> ScreenMirrorVideoCodec {
    ScreenMirrorVideoCodec {
        annex_b: text(value, "annexB"),
        key_frame: value["keyFrame"]
            .as_str()
            .filter(|key_frame| !key_frame.is_empty())
            .map(str::to_owned),
    }
}

fn quality(value: &Value) -> ScreenMirrorQuality {
    ScreenMirrorQuality {
        mode: ScreenMirrorMode::parse(&text(value, "mode")),
        resolution: integer(value, "resolution") as i32,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/screen_mirror/mod.rs"]
mod tests;

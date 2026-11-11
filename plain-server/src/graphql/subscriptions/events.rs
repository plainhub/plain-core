use crate::graphql::executor::{RequestClient, RequestOrigin};
use crate::server::ServerState;
use futures_util::{
    StreamExt,
    stream::{self, BoxStream},
};
use plain_graphql::{Context, Result};
use plain_rs::infrastructure::protocols::graphql::events::EVENTS;
use plain_rs::infrastructure::protocols::graphql::events::EventSpec;
use serde::de::DeserializeOwned;

pub(super) async fn authorize(
    state: &ServerState,
    origin: RequestOrigin,
    spec: &EventSpec,
) -> Result<()> {
    if let Some(capability) = spec.capability {
        plain_rs::features::services::capabilities::require(&state.host, capability)
            .await
            .map_err(|e| plain_graphql::Error::new(e.to_string()))?;
    }
    if !spec.access.is_empty() {
        let features: Vec<_> = spec
            .access
            .iter()
            .copied()
            .filter(|feature| {
                origin == RequestOrigin::Local
                    || plain_rs::features::services::device::access::is_enabled(
                        &state.prefs,
                        &[*feature],
                    )
            })
            .collect();
        if features.is_empty() {
            return Err("no_permission".into());
        }
        let granted = plain_rs::features::services::device::access::sample(&state.host, &features)
            .await
            .map_err(plain_graphql::Error::new)?;
        if granted.is_empty() {
            return Err("no_permission".into());
        }
    }
    Ok(())
}

async fn mirror_snapshot(state: &ServerState, name: &str) -> Result<String> {
    let snapshot = state
        .host
        .call("systemScreenMirrorState", plain_libs::serde_json::json!({}))
        .await
        .map_err(plain_graphql::Error::new)?;
    if name == "SCREEN_MIRRORING" {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Running {
            running: bool,
        }
        let running: Running = plain_libs::serde_json::from_value(snapshot)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        plain_libs::serde_json::to_string(&running)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))
    } else {
        #[derive(serde::Deserialize)]
        struct Codec {
            codec: Option<super::models::MirrorCodecConfiguration>,
        }
        let codec: Codec = plain_libs::serde_json::from_value(snapshot)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        plain_libs::serde_json::to_string(&codec.codec)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }
}

pub(super) async fn stream<T: DeserializeOwned + Send + 'static>(
    ctx: &Context<'_>,
    name: &'static str,
) -> Result<BoxStream<'static, Result<T>>> {
    let state = ctx.data::<ServerState>()?.clone();
    let origin = *ctx.data::<RequestOrigin>()?;
    let client = ctx.data::<RequestClient>()?.client_id().to_owned();
    let spec = EVENTS
        .iter()
        .find(|spec| spec.field == ctx.field().name())
        .ok_or("unknown_subscription")?;
    let receiver = state.events.subscribe();
    authorize(&state, origin, spec).await?;
    match spec.field {
        "smsSendResult" => state
            .sms_sends
            .replay()
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?,
        "mmsSendResult" => state
            .mms
            .replay()
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?,
        _ => {}
    }
    let initial = snapshot(&state, name).await?;
    let has_snapshot = initial.is_some();
    if let Some(ready) = ctx.data_opt::<super::session::SubscriptionReady>() {
        ready.0.send_replace(true);
    }
    let stop = state.stop.clone();
    Ok(stream::unfold((receiver, state, client, stop, initial, false), move |(mut receiver, state, client, mut stop, mut initial, ended)| async move {
        if ended { return None; }
        loop {
            let payload = if let Some(value) = initial.take() { value } else {
                let event = tokio::select! {
                    biased;
                    _ = async { let _ = stop.wait_for(|stopped| *stopped).await; } => return None,
                    event = receiver.recv() => match event {
                        Ok(event) => event,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Some((Err("resync_required".into()), (receiver, state, client, stop, None, true))),
                        Err(_) => return None,
                    },
                };
                if matches!(event.event_type, "PREFS_UPDATED" | "ACCESS_UPDATED") {
                    if let Err(error) = authorize(&state, origin, spec).await {
                        return Some((Err(error), (receiver, state, client, stop, None, true)));
                    }
                }
                if !spec.matches(event.event_type) || event.target_cid.as_ref().is_some_and(|target| target != &client) { continue; }
                if has_snapshot {
                    match snapshot(&state, name).await {
                        Ok(Some(snapshot)) => snapshot,
                        Ok(None) => unreachable!("subscription snapshot kind changed"),
                        Err(error) => return Some((Err(error), (receiver, state, client, stop, None, true))),
                    }
                } else { event.payload }
            };
            let authorized = authorize(&state, origin, spec).await;
            let result = authorized.and_then(|_| {
                let payload = if spec.invalidation { "true" } else { &payload };
                plain_libs::serde_json::from_str(payload).map_err(|e| plain_graphql::Error::new(format!("Invalid {name} event: {e}")))
            });
            let terminal = result.is_err();
            return Some((result, (receiver, state, client, stop, None, terminal)));
        }
    }).boxed())
}

async fn snapshot(state: &ServerState, name: &str) -> Result<Option<String>> {
    let value = match name {
        "SCREEN_MIRRORING" | "SCREEN_MIRROR_VIDEO_CODEC" => {
            return mirror_snapshot(state, name).await.map(Some);
        }
        "DLNA_SENDER_CHANGED" => plain_libs::serde_json::to_string(&state.cast.snapshot())
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?,
        "IMAGE_MODELS_UPDATED" => state.image_models.snapshot().event().payload,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

use super::registry::{EVENTS, EventSpec};
use crate::assembly::server::ServerState;
use crate::graphql::executor::{RequestClient, RequestOrigin};
use async_graphql::{Context, Result};
use futures_util::{
    StreamExt,
    stream::{self, BoxStream},
};
use serde::de::DeserializeOwned;

pub(super) async fn authorize(
    state: &ServerState,
    origin: RequestOrigin,
    spec: &EventSpec,
) -> Result<()> {
    if origin == RequestOrigin::Remote
        && !crate::http::auth::public_gate::is_enabled(&state.prefs, spec.permissions)
    {
        return Err("no_permission".into());
    }
    if let Some(capability) = spec.capability {
        crate::services::capabilities::require(&state.host, capability)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    }
    if !spec.permissions.is_empty() {
        let names = spec
            .permissions
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        if !crate::http::auth::public_gate::granted(&state.host, &names)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?
        {
            return Err("no_permission".into());
        }
    }
    Ok(())
}

async fn mirror_snapshot(state: &ServerState, name: &str) -> Result<String> {
    let snapshot = state
        .host
        .call("systemScreenMirrorState", serde_json::json!({}))
        .await
        .map_err(async_graphql::Error::new)?;
    if name == "SCREEN_MIRRORING" {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Running {
            running: bool,
        }
        let running: Running = serde_json::from_value(snapshot)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        serde_json::to_string(&running)
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    } else {
        #[derive(serde::Deserialize)]
        struct Codec {
            codec: Option<super::models::MirrorCodecConfiguration>,
        }
        let codec: Codec = serde_json::from_value(snapshot)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        serde_json::to_string(&codec.codec)
            .map_err(|error| async_graphql::Error::new(error.to_string()))
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
        "smsSendResult" => state.sms_sends.replay().map_err(|error| async_graphql::Error::new(error.to_string()))?,
        "mmsSendResult" => state.mobile.mms.replay().map_err(|error| async_graphql::Error::new(error.to_string()))?,
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
                if matches!(event.event_type, "PREFS_UPDATED" | "PERMISSIONS_UPDATED") {
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
                serde_json::from_str(payload).map_err(|e| async_graphql::Error::new(format!("Invalid {name} event: {e}")))
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
        "DLNA_SENDER_CHANGED" => serde_json::to_string(&state.cast.snapshot())
            .map_err(|error| async_graphql::Error::new(error.to_string()))?,
        "IMAGE_MODELS_UPDATED" => state.image_models.snapshot().event().payload,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

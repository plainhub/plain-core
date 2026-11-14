use crate::features::services::casting::dlna_sender_runtime::{DlnaCastType, Item};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::dlna::didl;
use plain_libs::dlna::soap as protocol;
use plain_libs::dlna::types::DiscoveredDevice;
use plain_libs::dlna::types::MediaType;
use plain_libs::dlna::util::xml_escape;
use plain_libs::serde_json::json;
use std::time::Duration;

pub async fn soap(
    state: &CoreRuntime,
    device: &DiscoveredDevice,
    action: &str,
    params: &str,
) -> Result<String, String> {
    let (_, name) = crate::features::services::casting::address::http_address(state).await?;
    protocol::soap_response(device, action, params, &name).await
}
async fn mime_type(state: &CoreRuntime, path: &str) -> String {
    crate::features::services::files::resources::read(&state.host, "mime", path, json!({}))
        .await
        .ok()
        .and_then(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| plain_libs::utils::mime::mime_from_ext(path).to_owned())
}

pub async fn cast(state: &CoreRuntime, item: Item, advance: bool) -> Result<(), String> {
    cast_with_mime(state, item, advance, None).await
}
pub async fn cast_with_mime(
    state: &CoreRuntime,
    item: Item,
    advance: bool,
    requested_mime: Option<String>,
) -> Result<(), String> {
    let runtime = state.cast.clone();
    let generation = runtime.media.generation();
    let device = runtime.device()?;
    let peer = runtime
        .snapshot()
        .current_device
        .ok_or("No cast device selected")?
        .host_address
        .parse()
        .map_err(|_| "Invalid cast device address")?;
    let (base, _) = crate::features::services::casting::address::http_address(state).await?;
    let mime = match requested_mime.filter(|mime| !mime.trim().is_empty()) {
        Some(mime) => mime,
        None => mime_type(state, &item.path).await,
    };
    let mut session = super::media_alias::Session::new(peer);
    let url = format!("{base}/dlna/media/{}", session.register(&item.path, &mime));
    let art = if item.album_art.is_empty() {
        String::new()
    } else {
        format!(
            "{base}/dlna/media/{}",
            session.register(&item.album_art, "image/jpeg")
        )
    };
    let pending = session.pending();
    if !runtime.media.replace_current(session, generation) {
        return Err("Cast canceled".into());
    }
    *runtime.transport.lock().unwrap() = super::dlna_sender_runtime::Transport {
        uri: url.clone(),
        item: Some(item.clone()),
        mime: mime.clone(),
        ..Default::default()
    };
    let sending = async {
        let media_type = match item.media_type {
            DlnaCastType::Audio => MediaType::Audio,
            DlnaCastType::Video => MediaType::Video,
            DlnaCastType::Image => MediaType::Image,
        };
        let meta = if item.title.is_empty() {
            String::new()
        } else {
            didl::metadata(
                &url,
                if item.title.trim().is_empty() {
                    "PlainApp"
                } else {
                    &item.title
                },
                &mime,
                media_type,
                &art,
                true,
            )
        };
        soap(
            state,
            &device,
            "SetAVTransportURI",
            &format!("<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>", xml_escape(&url), xml_escape(&meta)),
        ).await?;
        soap(
            state,
            &device,
            "Play",
            "<InstanceID>0</InstanceID><Speed>1</Speed>",
        )
        .await?;
        runtime.transport.lock().unwrap().started = true;
        if item.media_type == DlnaCastType::Audio {
            let track = item.clone();
            state
                .audio
                .run(move |db, _lib| {
                    crate::features::media::audio::queue::on_playing(db, &track.path)
                })
                .await
                .map_err(|error| error.to_string())?;
        }
        if !advance {
            unsubscribe(state).await;
        }
        {
            let mut s = runtime.snapshot.lock().unwrap();
            s.current_audio = item.media_type == DlnaCastType::Audio;
            s.current_uri = item.path;
            s.playing = true;
            s.active = true;
        }
        if !advance {
            let (base, _) =
                crate::features::services::casting::address::http_address(state).await?;
            let callback = format!("{base}/dlna/callback");
            if let Ok(subscription) =
                protocol::subscription(&device, Some(&callback), "", false).await
            {
                if !subscription.sid.is_empty() {
                    *runtime.subscription_device.lock().unwrap() = Some(device.clone());
                    *runtime.subscription_renew_after.lock().unwrap() = subscription.renew_after;
                    *runtime.callback_sequence.lock().unwrap() = None;
                    let mut s = runtime.snapshot.lock().unwrap();
                    s.sid = subscription.sid;
                    s.supports_callback = true;
                }
            }
            start_tasks(state);
        }
        runtime.publish(state);
        Ok(())
    };
    let outcome = tokio::select! {
        biased;
        _ = pending.canceled() => Err("Cast canceled".to_owned()),
        outcome = sending => outcome,
    };
    if outcome.is_err() {
        runtime.media.revoke();
        {
            let mut snapshot = runtime.snapshot.lock().unwrap();
            snapshot.playing = false;
            snapshot.active = false;
            snapshot.current_uri.clear();
        }
        runtime.transport.lock().unwrap().uri.clear();
    } else {
        pending.keep();
    }
    outcome
}
async fn unsubscribe(state: &CoreRuntime) {
    let runtime = &state.cast;
    if let Some(task) = runtime.renewal.lock().unwrap().take() {
        task.abort();
    }
    let sid = {
        let mut s = runtime.snapshot.lock().unwrap();
        s.supports_callback = false;
        std::mem::take(&mut s.sid)
    };
    let device = runtime.subscription_device.lock().unwrap().take();
    *runtime.callback_sequence.lock().unwrap() = None;
    if let Some(device) = device.filter(|_| !sid.is_empty()) {
        let _ = protocol::subscription(&device, None, &sid, true).await;
    }
}
pub async fn end(state: &CoreRuntime, stop: bool) -> bool {
    let runtime = &state.cast;
    let generation = if stop {
        runtime.media.revoke()
    } else {
        runtime.media.generation()
    };
    if stop {
        *runtime.transport.lock().unwrap() = Default::default();
        if let Some(task) = runtime.polling.lock().unwrap().take() {
            task.abort();
        }
    }
    if let Ok(device) = runtime.device() {
        if stop {
            let _ = soap(state, &device, "Stop", "<InstanceID>0</InstanceID>").await;
        }
    }
    unsubscribe(state).await;
    let mut s = runtime.snapshot.lock().unwrap();
    s.playing = false;
    s.progress_ms = 0;
    s.duration_ms = 0;
    s.supports_callback = false;
    if stop {
        s.items.clear();
        s.current_uri.clear();
        s.current_audio = false;
        s.current_device = None;
        s.active = false;
    }
    runtime.media.generation() == generation
}
fn start_tasks(state: &CoreRuntime) {
    let runtime = &state.cast;
    if let Some(task) = runtime.polling.lock().unwrap().take() {
        task.abort();
    }
    let poll_state = state.clone();
    *runtime.polling.lock().unwrap() = Some(tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let runtime = &poll_state.cast;
            let _guard = runtime.operations.lock().await;
            let snap = runtime.snapshot();
            let generation = runtime.media.generation();
            if !snap.active || snap.current_uri.is_empty() {
                break;
            }
            let Ok(device) = runtime.device() else {
                break;
            };
            let mut fields = std::collections::HashMap::new();
            if let Ok(xml) = soap(
                &poll_state,
                &device,
                "GetPositionInfo",
                "<InstanceID>0</InstanceID>",
            )
            .await
            {
                fields.extend(xml_fields(&xml));
            }
            if let Ok(xml) = soap(
                &poll_state,
                &device,
                "GetTransportInfo",
                "<InstanceID>0</InstanceID>",
            )
            .await
            {
                fields.extend(xml_fields(&xml));
            }
            if let Some(value) = fields.remove("CurrentTransportState") {
                fields.insert("TransportState".into(), value);
            }
            if runtime.media.generation() != generation {
                continue;
            }
            if let Err(error) = super::cast_runtime::apply_poll(&poll_state, &fields).await {
                log::warn!("Cast state update failed: {error}");
            }
            runtime.publish(&poll_state);
        }
    }));
    if runtime.snapshot().sid.is_empty() {
        return;
    }
    let renew_state = state.clone();
    *runtime.renewal.lock().unwrap() = Some(tokio::spawn(async move {
        loop {
            let delay = *renew_state.cast.subscription_renew_after.lock().unwrap();
            tokio::time::sleep(delay).await;
            let runtime = &renew_state.cast;
            let _guard = runtime.operations.lock().await;
            let Ok(device) = runtime.device() else {
                break;
            };
            let snapshot = runtime.snapshot();
            let sid = snapshot.sid;
            if !snapshot.active || sid.is_empty() || snapshot.current_uri.is_empty() {
                break;
            }
            match protocol::subscription(&device, None, &sid, false).await {
                Ok(next) => {
                    *runtime.subscription_renew_after.lock().unwrap() = next.renew_after;
                    if !next.sid.is_empty() {
                        runtime.snapshot.lock().unwrap().sid = next.sid;
                    }
                }
                Err(_) => {
                    runtime.snapshot.lock().unwrap().supports_callback = false;
                }
            }
            runtime.publish(&renew_state);
        }
    }));
}
fn xml_fields(xml: &str) -> std::collections::HashMap<String, String> {
    use plain_libs::dlna::xml_sax::Event;
    use plain_libs::dlna::xml_sax::Reader;
    let mut reader = Reader::new(xml.as_bytes());
    let mut path = Vec::new();
    let mut values = std::collections::HashMap::new();
    while let Some(event) = reader.next_event() {
        match event {
            Event::Start { name } => path.push(name),
            Event::End { .. } => {
                path.pop();
            }
            Event::Text { content } => {
                if let Some(name) = path.last() {
                    values.insert(name.clone(), content);
                }
            }
        }
    }
    values
}

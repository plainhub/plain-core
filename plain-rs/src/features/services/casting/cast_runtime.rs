use crate::runtime::Runtime as CoreRuntime;

pub fn time_ms(time: &str) -> i64 {
    // Mobile playback reports whole seconds even when the renderer includes fractions.
    let time = time.split('.').next().unwrap_or_default();
    crate::features::services::casting::receiver::soap_handler::parse_dlna_time_to_ms(time).max(0)
}
pub fn attributes(xml: &str) -> std::collections::HashMap<String, String> {
    use plain_libs::utils::xml::Event;
    use plain_libs::utils::xml::Reader;
    let mut values = std::collections::HashMap::new();
    let mut documents = vec![xml.to_owned()];
    let mut remaining = 8;
    while let Some(document) = documents.pop() {
        if remaining == 0 {
            break;
        }
        remaining -= 1;
        let mut reader = Reader::new(&document);
        let mut last_change = None::<String>;
        while let Some(event) = reader.next() {
            match event {
                Event::Start(tag) => {
                    // Keys keep the casing the sender used — the callback
                    // looks them up as `TrackDuration`, not `trackduration`.
                    if let Some(val) = tag.attr("val") {
                        values.insert(tag.raw.clone(), val.to_owned());
                    }
                    if tag.name == "avtransporturimetadata" {
                        values.entry(tag.raw.clone()).or_default();
                    }
                    if tag.name == "lastchange" {
                        last_change = Some(String::new());
                    }
                }
                Event::Text(text) => {
                    if let Some(body) = last_change.as_mut() {
                        body.push_str(&text);
                    }
                }
                Event::End(name) if name == "lastchange" => {
                    if let Some(body) = last_change.take().filter(|body| body.contains('<')) {
                        documents.push(body);
                    }
                }
                _ => {}
            }
        }
    }
    values
}
pub async fn apply(
    state: &CoreRuntime,
    fields: &std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    observe(state, fields, false).await
}

pub async fn apply_poll(
    state: &CoreRuntime,
    fields: &std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    observe(state, fields, true).await
}

async fn observe(
    state: &CoreRuntime,
    fields: &std::collections::HashMap<String, String>,
    polled: bool,
) -> anyhow::Result<()> {
    let runtime = &state.cast;
    let status = fields.get("TransportState").map(String::as_str);
    let ended = {
        let mut transport = runtime.transport.lock().unwrap();
        if transport.uri.is_empty() {
            return Ok(());
        }
        if fields
            .get("TrackURI")
            .filter(|uri| !uri.is_empty())
            .is_some_and(|uri| uri != &transport.uri)
        {
            return Ok(());
        }
        match status {
            Some("PLAYING" | "PAUSED_PLAYBACK") => {
                transport.started = true;
                transport.stopped = false;
                runtime.media.renew();
                false
            }
            Some("STOPPED" | "NO_MEDIA_PRESENT") if polled => {
                let ended = transport.started && transport.stopped;
                transport.stopped = true;
                ended
            }
            Some("TRANSITIONING") => {
                transport.stopped = false;
                false
            }
            _ => false,
        }
    };
    {
        let mut s = runtime.snapshot.lock().unwrap();
        if let (Some(position), Some(duration)) =
            (fields.get("RelTime"), fields.get("TrackDuration"))
        {
            s.progress_ms = time_ms(position);
            s.duration_ms = time_ms(duration);
        }
        match status {
            Some("PLAYING") => s.playing = true,
            Some("PAUSED_PLAYBACK") => s.playing = false,
            _ => {}
        }
    }
    if ended {
        runtime.media.revoke();
        runtime.transport.lock().unwrap().uri.clear();
        runtime.snapshot.lock().unwrap().playing = false;
        let snapshot = runtime.snapshot();
        let next = if !snapshot.items.is_empty() {
            let index = snapshot
                .items
                .iter()
                .position(|item| item.path == snapshot.current_uri)
                .map_or(0, |index| (index + 1) % snapshot.items.len());
            Some(snapshot.items[index].clone())
        } else if snapshot.current_audio {
            let shuffle = crate::features::media::audio::queue::play_mode(&state.prefs)?
                == crate::features::media::audio::play_mode::MediaPlayMode::Shuffle;
            state
                .audio
                .run(move |db, _| {
                    crate::features::media::audio::queue::select_next(db, true, shuffle)
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .map(
                    |track| crate::features::services::casting::dlna_sender_runtime::Item {
                        path: track.path,
                        title: track.title,
                        audio: true,
                        ..Default::default()
                    },
                )
        } else {
            None
        };
        if let Some(next) =
            next.filter(|item| !item.path.is_empty() && item.path != snapshot.current_uri)
        {
            crate::features::services::casting::dlna_sender_playback::cast(state, next, true)
                .await
                .map_err(anyhow::Error::msg)?;
        } else {
            super::dlna_sender_playback::end(state, false).await;
            runtime.snapshot.lock().unwrap().active = false;
        }
    }
    runtime.publish(state);
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/casting/cast_runtime.rs"]
mod tests;

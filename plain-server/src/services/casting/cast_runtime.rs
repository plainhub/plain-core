use crate::assembly::server::ServerState;

pub(crate) fn time_ms(time: &str) -> i64 {
    // Mobile playback reports whole seconds even when the renderer includes fractions.
    let time = time.split('.').next().unwrap_or_default();
    crate::services::casting::receiver::soap_handler::parse_dlna_time_to_ms(time).max(0)
}
pub(crate) fn attributes(xml: &str) -> std::collections::HashMap<String, String> {
    use plain_rs::utils::xml::Event;
    use plain_rs::utils::xml::Reader;
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
pub(crate) async fn apply(
    state: &ServerState,
    fields: &std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    let runtime = &state.cast;
    {
        let mut s = runtime.snapshot.lock().unwrap();
        if let (Some(position), Some(duration)) =
            (fields.get("RelTime"), fields.get("TrackDuration"))
        {
            s.progress_ms = time_ms(position);
            s.duration_ms = time_ms(duration);
        }
        match fields.get("TransportState").map(String::as_str) {
            Some("PLAYING") => s.playing = true,
            Some("PAUSED_PLAYBACK" | "STOPPED") => s.playing = false,
            _ => {}
        }
    }
    if fields.get("TransportState").map(String::as_str) == Some("STOPPED")
        && !fields.contains_key("AVTransportURIMetaData")
        && runtime.snapshot().current_device.is_some()
    {
        let snapshot = runtime.snapshot();
        let next = if !snapshot.items.is_empty() {
            let index = snapshot
                .items
                .iter()
                .position(|item| item.path == snapshot.current_uri)
                .map_or(0, |index| (index + 1) % snapshot.items.len());
            Some(snapshot.items[index].clone())
        } else if snapshot.current_audio {
            let shuffle = plain_rs::media::audio::queue::play_mode(&state.prefs)?
                == plain_rs::media::audio::play_mode::MediaPlayMode::Shuffle;
            state
                .audio
                .run(move |db, _| plain_rs::media::audio::queue::select_next(db, true, shuffle))
                .await
                .map_err(|e| anyhow::anyhow!(e.message))?
                .map(
                    |track| crate::services::casting::dlna_sender_runtime::Item {
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
            crate::services::casting::dlna_sender_playback::cast(state, next, true)
                .await
                .map_err(anyhow::Error::msg)?;
        }
    }
    runtime.publish(state);
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/casting/cast_runtime.rs"]
mod tests;

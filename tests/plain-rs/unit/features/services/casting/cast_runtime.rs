use super::*;
#[test]
fn callback_reads_escaped_last_change_without_advancing_metadata_duplicates() {
    let xml = r#"<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0"><e:property><LastChange>&lt;Event&gt;&lt;InstanceID val="0"&gt;&lt;TransportState val="STOPPED"/&gt;&lt;AVTransportURIMetaData val=""/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange></e:property></e:propertyset>"#;
    let values = attributes(xml);
    assert_eq!(values["TransportState"], "STOPPED");
    assert!(values.contains_key("AVTransportURIMetaData"));
}
#[test]
fn callback_reads_cdata_position_and_pause() {
    let values = attributes(
        r#"<LastChange><![CDATA[<Event><InstanceID val="0"><TransportState val="PAUSED_PLAYBACK"/><RelTime val="01:02:03.999"/><TrackDuration val="02:00:00"/></InstanceID></Event>]]></LastChange>"#,
    );
    assert_eq!(values["TransportState"], "PAUSED_PLAYBACK");
    assert_eq!(time_ms(&values["RelTime"]), 3723000);
    assert_eq!(time_ms(&values["TrackDuration"]), 7200000);
}
#[test]
fn malformed_or_unknown_position_is_zero() {
    for value in ["", "NOT_IMPLEMENTED", "bad", "-1:00:00"] {
        assert_eq!(time_ms(value), 0);
    }
}

#[test]
fn legacy_seek_times_keep_millisecond_units_and_ignore_fractions() {
    for (text, ms) in [
        ("00:00:00", 0),
        ("00:00:05", 5000),
        ("00:00:59", 59000),
        ("00:01:00", 60000),
        ("00:01:05", 65000),
        ("01:00:00", 3600000),
        ("01:01:01", 3661000),
        ("10:30:45", 37845000),
        ("00:00:05.999", 5000),
    ] {
        assert_eq!(time_ms(text), ms);
    }
}

#[tokio::test]
async fn pause_seek_and_transient_stopped_preserve_links_but_confirmed_end_revokes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(dir.path()).unwrap();
    let state = core.runtime_state();
    let peer = "192.0.2.10".parse().unwrap();
    let mut session = crate::features::services::casting::media_alias::Session::new(peer);
    let name = session.register("/fixture.mp4", "video/mp4");
    state.cast.media.replace(session);
    state.cast.transport.lock().unwrap().uri = "http://example.test/dlna/media/fixture.mp4".into();
    state.cast.snapshot.lock().unwrap().current_uri = "/fixture.mp4".into();
    let fields =
        |status: &str| std::collections::HashMap::from([("TransportState".into(), status.into())]);
    apply_poll(&state, &fields("PLAYING")).await.unwrap();
    apply(&state, &fields("STOPPED")).await.unwrap();
    assert!(state.cast.media.lookup(&name, peer).is_some());
    apply_poll(&state, &fields("STOPPED")).await.unwrap();
    assert!(state.cast.media.lookup(&name, peer).is_some());
    apply_poll(&state, &fields("TRANSITIONING")).await.unwrap();
    apply_poll(&state, &fields("PAUSED_PLAYBACK"))
        .await
        .unwrap();
    assert!(!state.cast.snapshot().playing);
    assert!(state.cast.media.lookup(&name, peer).is_some());
    apply_poll(&state, &fields("PLAYING")).await.unwrap();
    let stream = state.cast.media.lookup(&name, peer).unwrap();
    apply_poll(&state, &fields("STOPPED")).await.unwrap();
    assert!(state.cast.media.lookup(&name, peer).is_some());
    apply_poll(&state, &fields("STOPPED")).await.unwrap();
    assert!(state.cast.media.lookup(&name, peer).is_none());
    stream.canceled().await;
    assert_eq!(state.cast.snapshot().current_uri, "/fixture.mp4");
    assert!(!state.cast.snapshot().active);
    core.stop_core().await;
}

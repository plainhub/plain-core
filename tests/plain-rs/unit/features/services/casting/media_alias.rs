use super::*;

#[test]
fn resources_are_random_exact_and_scoped_to_one_renderer_session() {
    let registry = Registry::default();
    let peer = "192.0.2.10".parse().unwrap();
    let mut session = Session::new(peer);
    let name = session.register("/a/MOVIE.MP4", "video/mp4");
    let art = session.register("content://covers/42", "image/jpeg");
    let other = session.register("/a/MOVIE.MP4", "video/mp4");
    assert_ne!(name, other);
    assert_eq!(name.len(), 68);
    assert!(name[..64].bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(art.ends_with(".jpg"));
    registry.replace(session);
    for _ in 0..3 {
        let resource = registry.lookup(&name, peer).unwrap();
        assert_eq!(resource.path, "/a/MOVIE.MP4");
        assert_eq!(resource.mime, "video/mp4");
    }
    for invalid in [
        name.replace(".mp4", ".mp3"),
        format!("{name}.mp4"),
        name[..64].to_owned(),
    ] {
        assert!(registry.lookup(&invalid, peer).is_none());
    }
    assert!(
        registry
            .lookup(&name, "192.0.2.11".parse().unwrap())
            .is_none()
    );
    registry.revoke();
    assert!(registry.lookup(&name, peer).is_none());
    assert!(registry.lookup(&art, peer).is_none());
}

#[tokio::test]
async fn replacement_and_revocation_cancel_already_authorized_resources() {
    let registry = Registry::default();
    let peer = "192.0.2.10".parse().unwrap();
    let mut first = Session::new(peer);
    let name = first.register("/one.mp4", "video/mp4");
    registry.replace(first);
    let stream = registry.lookup(&name, peer).unwrap();
    let mut second = Session::new(peer);
    let next = second.register("/two.mp4", "video/mp4");
    registry.replace(second);
    stream.cancelled().await;
    assert!(registry.lookup(&name, peer).is_none());
    let stream = registry.lookup(&next, peer).unwrap();
    registry.revoke();
    stream.cancelled().await;
    assert!(registry.lookup(&next, peer).is_none());
}

#[tokio::test(start_paused = true)]
async fn requests_do_not_renew_the_lease_and_expired_sessions_cannot_revive() {
    let registry = Registry::default();
    let peer = "192.0.2.10".parse().unwrap();
    let mut session = Session::new(peer);
    let name = session.register("/movie.mp4", "video/mp4");
    registry.replace(session);
    let stream = registry.lookup(&name, peer).unwrap();
    tokio::time::advance(TTL - Duration::from_secs(1)).await;
    registry.renew();
    tokio::time::advance(TTL - Duration::from_secs(1)).await;
    assert!(registry.lookup(&name, peer).is_some());
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(registry.lookup(&name, peer).is_none());
    registry.renew();
    assert!(!registry.accepts(peer));
    stream.cancelled().await;
}

#[test]
fn extensions_use_the_resource_name_or_known_mime() {
    assert_eq!(
        extension("https://example.org/movie.MP4?signature=value", "video/mp4"),
        "mp4"
    );
    assert_eq!(extension("content://media/42", "audio/mpeg"), "mp3");
    assert_eq!(
        extension("/dotted.dir/file", "application/octet-stream"),
        "bin"
    );
    assert_eq!(extension("/file.unsafe%20name", "video/mp4"), "mp4");
}

#[tokio::test]
async fn abandoned_cast_cancels_pending_media_even_when_registry_is_retained() {
    let registry = Registry::default();
    let peer = "192.0.2.10".parse().unwrap();
    let mut session = Session::new(peer);
    let name = session.register("/movie.mp4", "video/mp4");
    let pending = session.pending();
    registry.replace(session);
    let stream = registry.lookup(&name, peer).unwrap();
    drop(pending);
    assert!(registry.lookup(&name, peer).is_none());
    stream.cancelled().await;
}

#[test]
fn stop_prevents_an_earlier_cast_from_installing_late_media() {
    let registry = Registry::default();
    let generation = registry.generation();
    let session = Session::new("192.0.2.10".parse().unwrap());
    registry.revoke();
    assert!(!registry.replace_current(session, generation));
}

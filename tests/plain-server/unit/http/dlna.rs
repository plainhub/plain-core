use super::*;
use plain_libs::protocols::http_client;
use serde_json::{Value, json};
#[cfg(feature = "http_transport")]
const TOKEN: &str = "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=";

#[cfg(feature = "http_transport")]
fn fixture() -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap(),
    );
    let (key, _public) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", "actor").unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let host = server.runtime_state().host.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!([]),
                "mdnsMulticast" => json!(true),
                "systemCastAddressFacts" => {
                    json!({"ip":"127.0.0.1","defaultDeviceName":"Test Phone"})
                }
                other => panic!("Unexpected platform primitive: {other}"),
            };
            let _ = host.reply(generation, json!({"id":request["id"],"result":result}));
        }
    });
    assert_eq!(
        TOKEN,
        plain_libs::utils::base64::base64_encode(&[3; 32]),
        "token literal drifted"
    );
    (dir, server)
}

#[cfg(feature = "http_transport")]
async fn public_fixture() -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    u16,
) {
    let (dir, server) = fixture();
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (http, _) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
    (dir, server, http)
}

#[test]
fn snapshot_omits_the_internal_pending_slot() {
    let state =
        plain_rs::features::services::casting::receiver::renderer_state::DlnaRendererState::default(
        );
    let row = snapshot(&state);
    // The UI only ever needs the confirmed request; `raw_pending_cast_request`
    // is the rules input and must not leak into the host surface.
    assert!(row.get("rawPendingCastRequest").is_none());
    assert_eq!(row["pendingCastRequest"], Value::Null);
    assert_eq!(row["playbackState"], "NoMediaPresent");
    assert_eq!(row["seekTargetMs"], Value::Null);
}

#[test]
fn local_ip_skips_loopback_so_it_matches_the_ssdp_location() {
    // A loopback-only host still has to produce something usable rather than
    // an empty string in the device description.
    let ip = local_ip();
    assert!(!ip.is_empty());
    assert!(!ip.starts_with("127."), "picked {ip}");
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn receiver_paths_answer_only_while_the_toggle_and_service_are_on() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (dir, server, port) = public_fixture().await;
    let client = http_client::builder().build().unwrap();
    let base = format!("http://127.0.0.1:{port}");
    let state = server.runtime_state();

    let describe = || async {
        client
            .get(format!("{base}/dlna/description.xml"))
            .send()
            .await
            .unwrap()
            .status()
    };

    // Both prefs off, which is the default: the renderer must not be
    // discoverable or answerable at all.
    assert_eq!(describe().await, 404);

    state.prefs.set("background_service_enabled", true).unwrap();
    assert_eq!(describe().await, 404, "service alone is not enough");

    state.prefs.set("dlna", true).unwrap();
    let body = client
        .get(format!("{base}/dlna/description.xml"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("<root"),
        "expected a device description, got {body}"
    );
    assert!(body.contains(server.runtime_state().dlna.device_uuid()));
    // `ensure_identity` seeds the device-name preference from the OS during
    // core construction, so the receiver answers with that name rather than
    // the host `defaultDeviceName`; only presence is contractual here.
    assert!(
        body.contains("<friendlyName>"),
        "expected a friendly name element, got {body}"
    );

    // The service description documents are static and served the same way.
    for path in [
        "/dlna/av-transport/scpd.xml",
        "/dlna/rendering-control/scpd.xml",
    ] {
        assert_eq!(
            client
                .get(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            200,
            "{path}"
        );
    }
    // An unregistered path is not the receiver's business: it falls through to
    // the SPA/bridge fallback, and must not be answered as UPnP.
    let stray = client
        .get(format!("{base}/dlna/av-transport/nope"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        !stray.contains("<root"),
        "the receiver answered an unknown path"
    );
    server.shutdown().await;
    drop(dir);
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn a_forged_c_ip_header_cannot_impersonate_an_allowed_sender() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (dir, server, port) = public_fixture().await;
    let client = http_client::builder().build().unwrap();
    let base = format!("http://127.0.0.1:{port}");
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    state.prefs.set("dlna", true).unwrap();
    // 10.0.0.9 is "trusted"; the request below actually arrives from loopback.
    plain_rs::features::prefs::dlna::add_sender(
        &state.prefs,
        "dlna_allowed_senders",
        "10.0.0.9",
        "TV",
    );

    let soap = r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1"><InstanceID>0</InstanceID><CurrentURI>http://example/a.mp4</CurrentURI><CurrentURIMetaData/></u:SetAVTransportURI></s:Body></s:Envelope>"#;
    let status = client
        .post(format!("{base}/dlna/av-transport/control"))
        .header("content-type", "text/xml")
        .header(
            "soapaction",
            r#""urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI""#,
        )
        .header("c-ip", "10.0.0.9")
        .body(soap)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 200);

    // The trusted IP never spoke to us, so the request must not have been
    // auto-accepted — and what we recorded has to be the socket peer, not the
    // header the control point chose.
    let after = state.dlna.snapshot().await;
    let pending = after
        .pending_cast_request
        .expect("the request should be waiting for the user");
    assert_eq!(pending.sender_ip, "127.0.0.1");
    assert_eq!(pending.sender_name, "127.0.0.1");
    assert!(
        after.media_uri.is_empty(),
        "a forged c-ip header was trusted: {}",
        after.media_uri
    );
    server.shutdown().await;
    drop(dir);
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn native_control_uses_shared_http_lifecycle_and_cast_prompt() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (dir, server, port) = public_fixture().await;
    let controller = MainDlnaClient(server.native_graphql().unwrap(), server.runtime_state());
    let state = server.runtime_state();
    let post = |body: Value| {
        let controller = &controller;
        async move {
            let value: Value =
                serde_json::from_str(&controller.execute_json(&body.to_string()).await.unwrap())
                    .unwrap();
            (200, json!({"result": value}).to_string())
        }
    };

    assert!(
        controller
            .execute_json(r#"{"action":"position","positionMs":1}"#)
            .await
            .is_err()
    );
    assert!(
        controller
            .execute_json(r#"{"action":"snapshot","unexpected":true}"#)
            .await
            .is_err()
    );
    let (status, body) = post(json!({"action": "setEnabled", "enabled": true})).await;
    assert_eq!(status, 200);
    let row: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(row["result"]["isRunning"], json!(true), "{row}");
    assert_eq!(row["result"]["port"], json!(port));

    // Seed a pending request, then let the host accept it and remember the
    // sender — the same two-step the cast dialog performs.
    state.dlna.state.write().await.pending_cast_request = Some(
        plain_rs::features::services::casting::receiver::types::PendingCastRequest {
            sender_ip: "10.0.0.9".into(),
            sender_name: "TV".into(),
            media_uri: "http://example/a.mp4".into(),
            media_title: "a".into(),
            media_type:
                plain_rs::features::services::casting::receiver::types::DlnaMediaType::VIDEO,
            album_art_uri: String::new(),
        },
    );

    let (status, body) = post(json!({"action": "accept", "remember": true})).await;
    assert_eq!(status, 200);
    let row: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(row["result"]["pendingCastRequest"], Value::Null);
    assert_eq!(row["result"]["mediaUri"], json!("http://example/a.mp4"));
    assert!(plain_rs::features::prefs::dlna::senders_contain_ip(
        &plain_rs::features::prefs::dlna::senders(&state.prefs, "dlna_allowed_senders"),
        "10.0.0.9"
    ));

    // A rejected request lands the opposite way.
    state.dlna.state.write().await.pending_cast_request = Some(
        plain_rs::features::services::casting::receiver::types::PendingCastRequest {
            sender_ip: "10.0.0.8".into(),
            sender_name: "Other".into(),
            media_uri: "http://example/b.mp4".into(),
            media_title: "b".into(),
            media_type:
                plain_rs::features::services::casting::receiver::types::DlnaMediaType::AUDIO,
            album_art_uri: String::new(),
        },
    );
    let (status, body) = post(json!({"action": "reject", "remember": true})).await;
    assert_eq!(status, 200);
    let row: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(row["result"]["pendingCastRequest"], Value::Null);
    assert_ne!(row["result"]["mediaUri"], json!("http://example/b.mp4"));
    assert!(plain_rs::features::prefs::dlna::senders_contain_ip(
        &plain_rs::features::prefs::dlna::senders(&state.prefs, "dlna_denied_senders"),
        "10.0.0.8"
    ));

    let (status, body) =
        post(json!({"action": "position", "positionMs": 4200, "durationMs": 9000})).await;
    assert_eq!(status, 200, "position failed: {body}");
    let row: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
    assert_eq!(row["result"]["currentPositionMs"], json!(4200));
    assert_eq!(row["result"]["durationMs"], json!(9000));

    let rules: Value = serde_json::from_str(
        &controller
            .execute_json(r#"{"action":"rules"}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rules["allowed"][0]["ip"], "10.0.0.9");
    let rules: Value = serde_json::from_str(
        &controller
            .execute_json(r#"{"action":"removeRule","ip":"10.0.0.9","allowed":true}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rules["allowed"], json!([]));
    post(json!({"action":"playback","state":"PausedPlayback"})).await;
    state.dlna.state.write().await.seek_target_ms = Some(3000);
    post(json!({"action":"seekConsumed"})).await;
    let snapshot: Value = serde_json::from_str(
        &controller
            .execute_json(r#"{"action":"snapshot"}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot["playbackState"], "PausedPlayback");
    assert_eq!(snapshot["seekTargetMs"], Value::Null);
    post(json!({"action":"stopped"})).await;
    let snapshot: Value = serde_json::from_str(
        &controller
            .execute_json(r#"{"action":"snapshot"}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot["mediaUri"], "");
    post(json!({"action":"retry"})).await;
    let (_, body) = post(json!({"action": "setEnabled", "enabled": false})).await;
    let row: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(row["result"]["isRunning"], json!(false));
    server.shutdown().await;
    drop(dir);
}

#[tokio::test]
async fn public_dlna_reads_the_native_players_reported_position() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (_dir, server, port) = public_fixture().await;
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    state.prefs.set("dlna", true).unwrap();
    state.dlna.state.write().await.media_uri = "http://example.test/owned.mp4".into();
    let controller = MainDlnaClient(server.native_graphql().unwrap(), server.runtime_state());
    controller
        .execute_json(r#"{"action":"position","positionMs":15000,"durationMs":120000}"#)
        .await
        .unwrap();
    let soap = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetPositionInfo xmlns:u="urn:schemas-upnp-org:service:AVTransport:1"><InstanceID>0</InstanceID></u:GetPositionInfo></s:Body></s:Envelope>"#;
    let response = http_client::builder()
        .build()
        .unwrap()
        .post(format!("http://127.0.0.1:{port}/dlna/av-transport/control"))
        .header(
            "soapaction",
            r#""urn:schemas-upnp-org:service:AVTransport:1#GetPositionInfo""#,
        )
        .body(soap)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = response.text().await.unwrap();
    assert!(body.contains("<RelTime>00:00:15</RelTime>"), "{body}");
    assert!(
        body.contains("<TrackDuration>00:02:00</TrackDuration>"),
        "{body}"
    );
    server.shutdown().await;
}

#[tokio::test]
async fn a_body_at_the_cap_is_read_and_one_byte_over_is_not() {
    let at_cap =
        axum::body::Body::from_stream(futures_util::stream::iter(vec![Ok::<_, std::io::Error>(
            vec![0u8; MAX_BODY],
        )]));
    assert_eq!(read_body(at_cap, MAX_BODY).await.unwrap().len(), MAX_BODY);

    let over =
        axum::body::Body::from_stream(futures_util::stream::iter(vec![Ok::<_, std::io::Error>(
            vec![0u8; MAX_BODY + 1],
        )]));
    assert_eq!(read_body(over, MAX_BODY).await, Err(BodyFailure::TooLarge));
}

#[tokio::test]
async fn a_body_that_fails_mid_read_is_reported_as_unreadable() {
    let stream = futures_util::stream::iter(vec![
        Ok(vec![1u8, 2, 3]),
        Err(std::io::Error::other("peer reset")),
    ]);
    assert_eq!(
        read_body(axum::body::Body::from_stream(stream), MAX_BODY).await,
        Err(BodyFailure::Unreadable)
    );
}

#[tokio::test]
async fn the_two_body_faults_answer_differently() {
    assert_eq!(
        StatusCode::from(BodyFailure::TooLarge),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        StatusCode::from(BodyFailure::Unreadable),
        StatusCode::BAD_REQUEST
    );
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn an_oversized_control_request_never_reaches_the_protocol() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (dir, server, port) = public_fixture().await;
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    state.prefs.set("dlna", true).unwrap();

    let soap = format!(
        r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1"><InstanceID>0</InstanceID><CurrentURI>http://example/{}</CurrentURI><CurrentURIMetaData/></u:SetAVTransportURI></s:Body></s:Envelope>"#,
        "a".repeat(MAX_BODY)
    );
    assert!(soap.len() > MAX_BODY, "the request must exceed the cap");

    let status = http_client::builder()
        .build()
        .unwrap()
        .post(format!("http://127.0.0.1:{port}/dlna/av-transport/control"))
        .header("content-type", "text/xml")
        .header(
            "soapaction",
            r#""urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI""#,
        )
        .body(soap)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    // Before the cap was enforced the same request arrived as an empty SOAP
    // body, which the router answered and the command channel carried.
    let after = state.dlna.snapshot().await;
    assert!(after.pending_cast_request.is_none());
    assert!(after.media_uri.is_empty());
    server.shutdown().await;
    drop(dir);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn receiver_name_uses_the_native_default_and_current_core_setting_without_host_calls() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    state.prefs.set("device_name", "").unwrap();
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    assert_eq!(
        device_name(&state).await.unwrap(),
        plain_rs::features::prefs::identity::default_device_name()
    );
    state.prefs.set("device_name", "Shared Renderer").unwrap();
    assert_eq!(device_name(&state).await.unwrap(), "Shared Renderer");
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
}

struct MainDlnaClient(
    crate::native::graphql::NativeGraphqlExecutor,
    crate::server::ServerState,
);
impl MainDlnaClient {
    async fn execute_json(&self, command: &str) -> Result<String, String> {
        let mut input: Value = serde_json::from_str(command).map_err(|error| error.to_string())?;
        let action = input["action"].as_str().ok_or("Missing action")?.to_owned();
        if action == "setEnabled" {
            let enabled = input["enabled"].as_bool().ok_or("Missing enabled")?;
            let response = self
                .0
                .execute(
                    plain_graphql::Request::new(
                        "mutation($value:JSON!){setPref(key:\"dlna\",value:$value)}",
                    )
                    .variables(plain_graphql::Variables::from_json(
                        json!({"value":enabled}),
                    )),
                )
                .await;
            if let Some(error) = response.errors.first() {
                return Err(error.message.clone());
            }
            let mut value = plain_rs::features::services::casting::receiver::control::snapshot(
                &self.1.dlna.snapshot().await,
            );
            value["enabled"] = json!(plain_rs::features::prefs::dlna::enabled(&self.1.prefs));
            return serde_json::to_string(&value).map_err(|error| error.to_string());
        }
        let report_kind = match action.as_str() {
            "position" => Some("DLNA_POSITION_REPORTED"),
            "playback" => Some("DLNA_PLAYBACK_REPORTED"),
            "seekConsumed" => Some("DLNA_SEEK_CONSUMED_REPORTED"),
            "stopped" => Some("DLNA_STOPPED_REPORTED"),
            _ => None,
        };
        if let Some(kind) = report_kind {
            input.as_object_mut().unwrap().remove("action");
            let renderer = self.1.dlna.state.read().await;
            input["mediaUri"] = json!(renderer.media_uri);
            if kind == "DLNA_SEEK_CONSUMED_REPORTED" {
                input["positionMs"] = json!(renderer.seek_target_ms.unwrap());
            }
            drop(renderer);
            plain_rs::features::services::casting::receiver::reports::accept(
                &self.1,
                self.1.host.generation().unwrap(),
                kind,
                &input.to_string(),
            )
            .await?;
            return serde_json::to_string(
                &plain_rs::features::services::casting::receiver::control::snapshot(
                    &self.1.dlna.snapshot().await,
                ),
            )
            .map_err(|error| error.to_string());
        }
        use plain_rs::features::services::casting::receiver::control::Request;
        let request: Request = serde_json::from_value(input).map_err(|error| error.to_string())?;
        let management = crate::native::management::NativeManagement::new(self.1.clone());
        let result = match request {
            Request::Snapshot {} => management.dlna_receiver().await?,
            Request::Rules {} => management.dlna_receiver().await?["rules"].clone(),
            Request::Retry {} => management.retry_dlna_receiver().await?,
            Request::RemoveRule { ip, allowed } => management.remove_dlna_rule(ip, allowed).await?,
            Request::Accept { remember } => management.accept_dlna_cast(remember).await?,
            Request::Reject { remember } => management.reject_dlna_cast(remember).await?,
        };
        serde_json::to_string(&result).map_err(|error| error.to_string())
    }
}

#[tokio::test]
async fn media_get_head_range_repeats_and_revocation_use_the_registered_route() {
    let (_dir, server, port) = public_fixture().await;
    let state = server.runtime_state();
    let path = state.directory.join("movie.mp4");
    std::fs::write(&path, b"0123456789").unwrap();
    let mut session = plain_rs::features::services::casting::media_alias::Session::new(
        "127.0.0.1".parse().unwrap(),
    );
    let name = session.register(path.to_str().unwrap(), "video/mp4");
    state.cast.media.replace(session);
    let url = format!("http://127.0.0.1:{port}/dlna/media/{name}");
    let client = http_client::builder().no_proxy().build().unwrap();
    for _ in 0..2 {
        let response = client
            .request(http_client::Method::HEAD, &url)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-length"], "10");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.bytes().await.unwrap().is_empty());
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(&response.bytes().await.unwrap()[..], b"0123456789");
        let response = client
            .get(&url)
            .header("Range", "bytes=3-5")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-range"], "bytes 3-5/10");
        assert_eq!(&response.bytes().await.unwrap()[..], b"345");
    }
    let response = client
        .get(&url)
        .header("Range", "bytes=10-")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 416);
    assert_eq!(response.headers()["content-range"], "bytes */10");
    assert_eq!(
        client
            .get(url.replace(".mp4", ".mp3"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    state.cast.media.revoke();
    for method in [http_client::Method::GET, http_client::Method::HEAD] {
        assert_eq!(
            client
                .request(method, &url)
                .header("Range", "bytes=0-1")
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    server.shutdown().await;
}

#[tokio::test]
async fn media_revocation_cancels_an_open_file_body_and_denies_another_peer() {
    let (dir, server) = fixture();
    let state = server.runtime_state();
    let path = dir.path().join("movie.mp4");
    std::fs::write(&path, vec![7u8; 256 * 1024]).unwrap();
    let mut session = plain_rs::features::services::casting::media_alias::Session::new(
        "127.0.0.1".parse().unwrap(),
    );
    let name = session.register(path.to_str().unwrap(), "video/mp4");
    state.cast.media.replace(session);
    let request = || {
        HttpRequest::builder()
            .body(axum::body::Body::empty())
            .unwrap()
    };
    let response = media(
        State(state.clone()),
        ConnectInfo("127.0.0.2:1234".parse().unwrap()),
        Path(name.clone()),
        request(),
    )
    .await;
    assert_eq!(response.status(), 404);
    let response = media(
        State(state.clone()),
        ConnectInfo("127.0.0.1:1234".parse().unwrap()),
        Path(name),
        request(),
    )
    .await;
    assert_eq!(response.status(), 200);
    let mut body = response.into_body().into_data_stream();
    assert_eq!(body.next().await.unwrap().unwrap().len(), 64 * 1024);
    state.cast.media.revoke();
    assert!(body.next().await.is_none());
    server.shutdown().await;
}

#[test]
fn remote_proxy_rejects_local_special_and_metadata_addresses() {
    for address in [
        "127.0.0.1",
        "0.0.0.1",
        "169.254.169.254",
        "100.100.100.200",
        "168.63.129.16",
        "fd00:ec2::254",
        "224.0.0.1",
        "255.255.255.255",
        "::1",
        "::",
        "fe80::1",
        "ff02::1",
        "::ffff:127.0.0.1",
    ] {
        assert!(
            !remote_address_allowed(address.parse().unwrap()),
            "{address}"
        );
    }
    assert!(remote_address_allowed("192.0.2.15".parse().unwrap()));
}

#[tokio::test]
async fn callback_requires_current_device_sid_and_sequence() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    let session = plain_rs::features::services::casting::media_alias::Session::new(
        "127.0.0.1".parse().unwrap(),
    );
    state.cast.media.replace(session);
    state.cast.snapshot.lock().unwrap().sid = "uuid:fixture".into();
    let body = "<Event><TransportState val=\"PLAYING\"/></Event>";
    for (sid, sequence, peer, expected) in [
        (None, Some("0"), "127.0.0.1:1234", 412),
        (Some("uuid:fixture"), None, "127.0.0.1:1234", 412),
        (Some("uuid:other"), Some("0"), "127.0.0.1:1234", 412),
        (Some("uuid:fixture"), Some("0"), "127.0.0.2:1234", 412),
        (Some("uuid:fixture"), Some("0"), "127.0.0.1:1234", 200),
        (Some("uuid:fixture"), Some("0"), "127.0.0.1:1234", 200),
    ] {
        let mut request = HttpRequest::builder().method("NOTIFY");
        if let Some(sid) = sid {
            request = request.header("SID", sid);
        }
        if let Some(sequence) = sequence {
            request = request.header("SEQ", sequence);
        }
        let response = callback(
            State(state.clone()),
            ConnectInfo(peer.parse().unwrap()),
            request.body(axum::body::Body::from(body)).unwrap(),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
    state.cast.media.revoke();
    let request = HttpRequest::builder()
        .method("NOTIFY")
        .header("SID", "uuid:fixture")
        .header("SEQ", "1")
        .body(axum::body::Body::from(body))
        .unwrap();
    assert_eq!(
        callback(
            State(state.clone()),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            request
        )
        .await
        .status(),
        412
    );
    server.shutdown().await;
}

#[tokio::test]
async fn native_media_head_range_and_cancel_preserve_the_descriptor_extent() {
    use plain_rs::infrastructure::host::resource_stream::NativeResourceFile;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("native.bin");
    std::fs::write(
        &path,
        [
            b"prefix".as_slice(),
            &vec![5u8; 192 * 1024],
            b"suffix".as_slice(),
        ]
        .concat(),
    )
    .unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let host = state.host.clone();
    let resources = state.resources.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let provider = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            assert_eq!(request["method"], "fileResourceStream");
            let stream = resources
                .attach_native(request["params"]["id"].as_str().unwrap(), generation)
                .unwrap();
            host.reply(generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            let metadata: Value =
                serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
            assert_eq!(metadata["file"], true);
            stream
                .file(NativeResourceFile {
                    file: std::fs::File::open(&path).unwrap(),
                    offset: 6,
                    length: 192 * 1024,
                })
                .await
                .unwrap();
        }
    });
    let mut session = plain_rs::features::services::casting::media_alias::Session::new(
        "127.0.0.1".parse().unwrap(),
    );
    let name = session.register("content://fixture/movie", "video/mp4");
    state.cast.media.replace(session);
    for head in [true, false] {
        let request = HttpRequest::builder()
            .method(if head { "HEAD" } else { "GET" })
            .body(axum::body::Body::empty())
            .unwrap();
        let response = media(
            State(state.clone()),
            ConnectInfo("127.0.0.1:1234".parse().unwrap()),
            Path(name.clone()),
            request,
        )
        .await;
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-length"],
            (192 * 1024).to_string()
        );
        assert_eq!(response.headers()["content-type"], "video/mp4");
        let mut stream = response.into_body().into_data_stream();
        if head {
            assert!(stream.next().await.is_none());
        } else {
            assert_eq!(
                stream.next().await.unwrap().unwrap()[..],
                vec![5; 64 * 1024]
            );
            state.cast.media.revoke();
            assert!(stream.next().await.is_none());
        }
    }
    provider.abort();
    server.shutdown().await;
}

#[tokio::test]
async fn remote_proxy_pins_dns_preserves_head_range_and_does_not_forward_credentials() {
    use axum::{Router, routing::any};
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = requests.clone();
    let router = Router::new().fallback(any(move |request: HttpRequest| {
        let captured = captured.clone();
        async move {
            captured
                .lock()
                .unwrap()
                .push((request.method().clone(), request.headers().clone()));
            if request.uri().path() == "/redirect" {
                return (
                    StatusCode::FOUND,
                    [(header::LOCATION, "http://127.0.0.1/private")],
                    "",
                )
                    .into_response();
            }
            (
                StatusCode::PARTIAL_CONTENT,
                [
                    ("content-type", "video/mp4"),
                    ("content-length", "3"),
                    ("content-range", "bytes 2-4/10"),
                    ("set-cookie", "private=value"),
                ],
                "234",
            )
                .into_response()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let url = http_client::Url::parse(&format!(
        "http://media.example.invalid:{}/movie",
        address.port()
    ))
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, "bytes=2-4".parse().unwrap());
    headers.insert(header::COOKIE, "private=cookie".parse().unwrap());
    headers.insert(header::AUTHORIZATION, "Bearer private".parse().unwrap());
    for method in [http_client::Method::HEAD, http_client::Method::GET] {
        let upstream = remote_fetch(url.clone(), &method, &headers, address)
            .await
            .unwrap();
        let response = remote_response(upstream, &method);
        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-length"], "3");
        assert!(!response.headers().contains_key("set-cookie"));
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            &bytes[..],
            if method == http_client::Method::HEAD {
                b"".as_slice()
            } else {
                b"234".as_slice()
            }
        );
    }
    let redirect = remote_fetch(
        url.join("/redirect").unwrap(),
        &http_client::Method::GET,
        &headers,
        address,
    )
    .await
    .unwrap();
    assert_eq!(
        redirect.status(),
        302,
        "Redirect must return for target validation"
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].0, http_client::Method::HEAD);
    for (_, headers) in requests.iter() {
        assert_eq!(headers["range"], "bytes=2-4");
        assert!(!headers.contains_key(header::COOKIE));
        assert!(!headers.contains_key(header::AUTHORIZATION));
    }
    task.abort();
}

#[test]
fn remote_redirect_cannot_rebind_public_dns_to_a_lan_service() {
    let public = ["192.0.2.50:80".parse().unwrap()];
    let private = "10.0.0.50:80".parse().unwrap();
    assert!(!remote_target_allowed(
        private,
        "media.example",
        "media.example",
        Some(&public)
    ));
    assert!(remote_target_allowed(
        private,
        "media.example",
        "media.example",
        None
    ));
    assert!(remote_target_allowed(
        private,
        "media.example",
        "media.example",
        Some(&[private])
    ));
    assert!(!remote_target_allowed(
        private,
        "other.example",
        "media.example",
        Some(&[private])
    ));
    assert!(!remote_target_allowed(
        "10.0.0.50:8080".parse().unwrap(),
        "media.example",
        "media.example",
        Some(&[private])
    ));
    let private_v6 = "[fd00:1234::50]:80".parse().unwrap();
    assert!(remote_target_allowed(
        private_v6,
        "fd00:1234::50",
        "[fd00:1234::50]",
        Some(&[private_v6])
    ));
}

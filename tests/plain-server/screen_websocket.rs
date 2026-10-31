#![cfg(all(feature = "core", feature = "http_transport"))]
use futures_util::StreamExt;
use plain_server::server::ContentServer;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

fn public_fixture() -> (
    tempfile::TempDir,
    ContentServer,
    Arc<plain_rs::prefs::Prefs>,
) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("desktop_access", true).unwrap();
    prefs.set("client_id", "screen-fixture").unwrap();
    let core = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[4; 32]),
        prefs.clone(),
    )
    .unwrap();
    (directory, core, prefs)
}
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::{Message as ClientMessage, client::IntoClientRequest};

#[tokio::test]
async fn image_editor_socket_authenticates_and_delivers_binary_updates() {
    let (_directory, core, prefs) = public_fixture();
    let key = plain_rs::utils::base64::base64_decode(&plain_rs::prefs::identity::ensure_url_token(
        &prefs,
    ));
    let client = prefs.get::<String>("client_id").unwrap().unwrap();
    let url = format!("ws://127.0.0.1:{}/image_editor?cid={client}", core.port);
    let (mut denied, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    denied
        .send(ClientMessage::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    assert!(matches!(
        denied.next().await.unwrap().unwrap(),
        ClientMessage::Close(_)
    ));

    let (mut socket, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let registration = plain_rs::crypto::xchacha_encrypt_raw(
        &key,
        chrono::Utc::now().timestamp_millis().to_string().as_bytes(),
    )
    .unwrap();
    let mut stream = plain_rs::protocols::websocket::Stream::new(&registration);
    socket
        .send(ClientMessage::Binary(registration.into()))
        .await
        .unwrap();
    let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else {
        panic!("Expected encrypted ready")
    };
    assert_eq!(
        stream.decode(&frame, &key).unwrap(),
        ("IMAGE_EDITOR_READY".into(), vec![])
    );
    let request = async_graphql::Request::new("mutation($id: ID!, $update: String!) { broadcastImageEditorUpdate(id: $id, update: $update) }")
        .variables(async_graphql::Variables::from_json(json!({"id":"project", "update":"AQID"})));
    let response = core.native_graphql().unwrap().execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let ClientMessage::Binary(frame) = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Expected encrypted update")
    };
    assert_eq!(
        stream.decode(&frame, &key).unwrap(),
        (
            "IMAGE_EDITOR_UPDATE".into(),
            b"\x07project\x01\x02\x03".to_vec()
        )
    );
    core.shutdown().await;
}

#[tokio::test]
async fn screen_transports_authenticated_media_and_controls_separately_from_subscriptions() {
    let (_directory, core, prefs) = public_fixture();
    let key = plain_rs::utils::base64::base64_decode(&plain_rs::prefs::identity::ensure_url_token(
        &prefs,
    ));
    let client = prefs.get::<String>("client_id").unwrap().unwrap();
    let bridge = Arc::new(core.host_events().unwrap());
    let host = bridge.clone();
    let (sent, mut received) = mpsc::channel(8);
    let mirror = Arc::new(std::sync::Mutex::new(json!({"running":false,"codec":null})));
    let mirror_host = mirror.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = host.next_request().await {
            let method = request["method"].as_str().unwrap();
            let result = match method {
                "systemWebLoginRequested"
                | "systemWebLoginClosed"
                | "systemWebLoginCompleted"
                | "systemWebLoginSnapshot"
                | "peerNotificationsChanged"
                | "peerClipboardChanged"
                | "onlineClientsChanged"
                | "httpServerStatusChanged"
                | "clipboardHistoryRefresh"
                | "dlnaRendererChanged"
                | "dlnaRendererRefresh"
                | "sharedFolderDownloadsRefresh"
                | "peerTransportsChanged"
                | "appUpdateDownloadChanged"
                | "dlnaSenderChanged"
                | "feedSyncChanged"
                | "audioPlaybackRefresh"
                | "audioLibraryRefresh"
                | "networkInterfacesChanged"
                | "systemWebActivity"
                | "pairingSucceeded" => json!(null),
                "systemCapabilities" => json!(["SCREEN_MIRROR"]),
                "systemScreenMirrorState" => mirror_host.lock().unwrap().clone(),
                "systemScreenMirrorControls" | "systemScreenMirrorResetTouch" => {
                    sent.send((method.to_owned(), request["params"].clone()))
                        .await
                        .unwrap();
                    json!(true)
                }
                method => panic!("Unexpected platform primitive: {method}"),
            };
            let _ = host.reply(json!({"id":request["id"],"result":result}));
        }
    });
    let mut request = format!("ws://127.0.0.1:{}/graphql?cid={client}", core.port)
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "plain-graphql-ws-v1".parse().unwrap(),
    );
    let (mut subscriptions, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let init = plain_rs::protocols::graphql::seal_init(
        &key,
        &client,
        serde_json::to_string(&json!({
            "timestampMillis":chrono::Utc::now().timestamp_millis(), "clientNonce":"01".repeat(32)
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    subscriptions
        .send(ClientMessage::Binary(init.clone().into()))
        .await
        .unwrap();
    let ClientMessage::Binary(ack) = subscriptions.next().await.unwrap().unwrap() else {
        panic!("Expected encrypted acknowledgement")
    };
    assert!(plain_rs::protocols::graphql::open_ack(&key, &client, &init, &ack).is_some());
    let mut subscription_stream = plain_rs::protocols::graphql::Stream::new(
        &client,
        &init,
        &ack,
        plain_rs::protocols::graphql::Role::Client,
    )
    .unwrap();
    for (id, query) in [
        (
            "codec",
            "subscription { screenMirrorVideoCodec { annexB } }",
        ),
        ("state", "subscription { screenMirrorState { running } }"),
    ] {
        let command = json!({"type":"subscribe", "id":id, "payload":{"query":query}}).to_string();
        subscriptions
            .send(ClientMessage::Binary(
                subscription_stream
                    .encode(command.as_bytes(), &key)
                    .unwrap()
                    .into(),
            ))
            .await
            .unwrap();
    }
    let mut ready = 0;
    while ready < 2 {
        let ClientMessage::Binary(frame) =
            tokio::time::timeout(Duration::from_secs(3), subscriptions.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
        else {
            panic!("Expected subscription frame")
        };
        let value: serde_json::Value =
            serde_json::from_slice(&subscription_stream.decode(&frame, &key).unwrap()).unwrap();
        if value["payload"]["extensions"]["subscriptionReady"] == true {
            ready += 1;
        }
    }
    let (mut screen, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{}/screen?cid={client}",
        core.port
    ))
    .await
    .unwrap();
    let registration = plain_rs::crypto::xchacha_encrypt_raw(
        &key,
        chrono::Utc::now().timestamp_millis().to_string().as_bytes(),
    )
    .unwrap();
    let mut screen_stream = plain_rs::protocols::websocket::Stream::new(&registration);
    screen
        .send(ClientMessage::Binary(registration.into()))
        .await
        .unwrap();
    screen
        .send(ClientMessage::Ping(vec![1].into()))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(3), screen.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        ClientMessage::Pong(_)
    ));
    for payload in [
        br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#.as_slice(),
        &[0x54, 1, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0],
    ] {
        let mut plain = chrono::Utc::now().timestamp_millis().to_be_bytes().to_vec();
        plain.extend_from_slice(payload);
        screen
            .send(ClientMessage::Binary(
                plain_rs::crypto::xchacha_encrypt_raw(&key, &plain)
                    .unwrap()
                    .into(),
            ))
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), received.recv())
                .await
                .unwrap()
                .unwrap()
                .0,
            "systemScreenMirrorControls"
        );
    }
    for (kind, payload) in [
        ("SCREEN_MIRROR_VIDEO", vec![0, 255, 1]),
        ("SCREEN_MIRROR_AUDIO", vec![2, 0, 3]),
    ] {
        bridge.publish_binary(kind, payload.clone()).unwrap();
        let message = tokio::time::timeout(Duration::from_secs(3), screen.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let ClientMessage::Binary(bytes) = message else {
            panic!("Expected encrypted media")
        };
        assert_eq!(
            screen_stream.decode(&bytes, &key).unwrap(),
            (kind.to_owned(), payload)
        );
    }
    *mirror.lock().unwrap() = json!({"running":false,"codec":{"annexB":"fixture"}});
    bridge
        .publish_text(
            "SCREEN_MIRROR_VIDEO_CODEC",
            r#"{"annexB":"fixture"}"#.into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let ClientMessage::Binary(frame) = subscriptions.next().await.unwrap().unwrap() else {
                panic!("Expected codec subscription")
            };
            let value: serde_json::Value =
                serde_json::from_slice(&subscription_stream.decode(&frame, &key).unwrap()).unwrap();
            if value["payload"]["data"]["screenMirrorVideoCodec"]["annexB"] == "fixture" {
                break;
            }
        }
    })
    .await
    .unwrap();
    let mut control = chrono::Utc::now().timestamp_millis().to_be_bytes().to_vec();
    control.extend_from_slice(br#"{"type":"screenMirrorControl","input":{"action":"HOME"}}"#);
    let control = plain_rs::crypto::xchacha_encrypt_raw(&key, &control).unwrap();
    screen
        .send(ClientMessage::Binary(control.clone().into()))
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), received.recv())
            .await
            .unwrap()
            .unwrap()
            .0,
        "systemScreenMirrorControls"
    );
    screen
        .send(ClientMessage::Binary(control.into()))
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), screen.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(message, ClientMessage::Close(Some(frame)) if frame.reason == "invalid_request")
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), received.recv())
            .await
            .unwrap()
            .unwrap()
            .0,
        "systemScreenMirrorResetTouch"
    );
    assert!(received.try_recv().is_err());
    for _ in 0..300 {
        bridge
            .publish_binary("SCREEN_MIRROR_VIDEO", vec![1, 2, 3])
            .unwrap();
    }
    *mirror.lock().unwrap() = json!({"running":true,"codec":{"annexB":"fixture"}});
    bridge
        .publish_text("SCREEN_MIRRORING", r#"{"running":true}"#.into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let ClientMessage::Binary(frame) = subscriptions.next().await.unwrap().unwrap() else {
                panic!("Expected mirror state subscription")
            };
            let value: serde_json::Value =
                serde_json::from_slice(&subscription_stream.decode(&frame, &key).unwrap()).unwrap();
            if value["payload"]["data"]["screenMirrorState"]["running"] == true {
                break;
            }
        }
    })
    .await
    .unwrap();
    core.shutdown().await;
    responder.await.unwrap();
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn screen_rejects_login_modes_missing_clients_invalid_keys_and_disabled_access() {
    let (_directory, core, prefs) = public_fixture();
    let client = prefs.get::<String>("client_id").unwrap().unwrap();
    for query in [
        String::new(),
        format!("cid={client}&auth=1"),
        format!("cid={client}&auth=2"),
        format!("cid={client}"),
    ] {
        let (mut screen, _) = tokio_tungstenite::connect_async(format!(
            "ws://127.0.0.1:{}/screen?{query}",
            core.port
        ))
        .await
        .unwrap();
        if query == format!("cid={client}") {
            screen
                .send(ClientMessage::Binary(vec![0; 40].into()))
                .await
                .unwrap();
        }
        let message = tokio::time::timeout(Duration::from_secs(3), screen.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(message, ClientMessage::Close(_)));
    }
    prefs.set("desktop_access", false).unwrap();
    let (mut screen, _) = tokio_tungstenite::connect_async(format!(
        "ws://127.0.0.1:{}/screen?cid={client}",
        core.port
    ))
    .await
    .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), screen.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(message, ClientMessage::Close(Some(frame)) if frame.reason == "desktop_access_disabled")
    );
    core.shutdown().await;
}

#[tokio::test]
async fn remote_subscriptions_enforce_access_and_deliver_pomodoro_updates() {
    let (_directory, core, _) = public_fixture();
    let session = core
        .native_management()
        .unwrap()
        .create_session("Remote subscription fixture".into())
        .unwrap();
    let key = plain_rs::base64_decode(&session.token);
    let client = session.client_id;
    let mut request = format!("ws://127.0.0.1:{}/graphql?cid={client}", core.port)
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "plain-graphql-ws-v1".parse().unwrap(),
    );
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let init = plain_rs::protocols::graphql::seal_init(
        &key,
        &client,
        json!({
            "timestampMillis":chrono::Utc::now().timestamp_millis(), "clientNonce":"02".repeat(32)
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    socket
        .send(ClientMessage::Binary(init.clone().into()))
        .await
        .unwrap();
    let ClientMessage::Binary(ack) = socket.next().await.unwrap().unwrap() else {
        panic!("Expected encrypted acknowledgement")
    };
    assert!(plain_rs::protocols::graphql::open_ack(&key, &client, &init, &ack).is_some());
    let mut stream = plain_rs::protocols::graphql::Stream::new(
        &client,
        &init,
        &ack,
        plain_rs::protocols::graphql::Role::Client,
    )
    .unwrap();
    let query = json!({"type":"subscribe","id":"private","payload":{"query":"subscription { pairingSucceeded { deviceId key } }"}}).to_string();
    socket
        .send(ClientMessage::Binary(
            stream.encode(query.as_bytes(), &key).unwrap().into(),
        ))
        .await
        .unwrap();
    let ClientMessage::Binary(frame) = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    else {
        panic!("Expected encrypted authorization response")
    };
    let response: serde_json::Value =
        serde_json::from_slice(&stream.decode(&frame, &key).unwrap()).unwrap();
    assert_eq!(response["id"], "private");
    assert_eq!(response["type"], "error");
    assert_eq!(
        response["payload"][0]["message"],
        "Unknown subscription field"
    );
    let query = json!({"type":"subscribe","id":"models","payload":{"query":"subscription { imageModelsUpdated { version status downloadProgress errorMessage modelSize modelDir isIndexing totalImages indexedImages } }"}}).to_string();
    socket
        .send(ClientMessage::Binary(
            stream.encode(query.as_bytes(), &key).unwrap().into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected encrypted image model update")
            };
            let response: serde_json::Value =
                serde_json::from_slice(&stream.decode(&frame, &key).unwrap()).unwrap();
            if response["id"] == "models"
                && response["payload"]["data"]["imageModelsUpdated"]["status"].is_string()
            {
                break;
            }
            assert_ne!(response["type"], "error");
        }
    })
    .await
    .unwrap();
    for (id, query) in [
        (
            "pomodoro",
            "subscription { pomodoroAction { today { state isRunning } completedState } }",
        ),
        ("settings", "subscription { pomodoroSettingsChanged }"),
    ] {
        let command = json!({"type":"subscribe", "id":id, "payload":{"query":query}}).to_string();
        socket
            .send(ClientMessage::Binary(
                stream.encode(command.as_bytes(), &key).unwrap().into(),
            ))
            .await
            .unwrap();
    }
    let mut ready = std::collections::HashSet::new();
    tokio::time::timeout(Duration::from_secs(3), async {
        while ready.len() < 2 {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected subscription response")
            };
            let response: serde_json::Value =
                serde_json::from_slice(&stream.decode(&frame, &key).unwrap()).unwrap();
            assert_ne!(response["type"], "error");
            if response["payload"]["extensions"]["subscriptionReady"] == true {
                ready.insert(response["id"].as_str().unwrap().to_owned());
            }
        }
    })
    .await
    .unwrap();
    let native = core.native_graphql().unwrap();
    let settings = json!({"workDurationMin":26,"shortBreakDurationMin":5,"longBreakDurationMin":15,"pomodorosBeforeLongBreak":4,"showNotification":true,"playSoundOnComplete":true,"soundPath":"","originalSoundName":""});
    let request = async_graphql::Request::new(
        "mutation($value: JSON!) { setPref(key: \"pomodoro_settings\", value: $value) }",
    )
    .variables(async_graphql::Variables::from_json(
        json!({"value":settings}),
    ));
    assert!(native.execute(request).await.errors.is_empty());
    assert!(
        native
            .execute("mutation { startPomodoro(durationSec:30) }".into())
            .await
            .errors
            .is_empty()
    );
    let mut received = std::collections::HashSet::new();
    tokio::time::timeout(Duration::from_secs(3), async {
        while received.len() < 2 {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else {
                panic!("Expected encrypted subscription update")
            };
            let response: serde_json::Value =
                serde_json::from_slice(&stream.decode(&frame, &key).unwrap()).unwrap();
            assert_ne!(response["type"], "error");
            if response["id"] == "settings"
                && response["payload"]["data"]["pomodoroSettingsChanged"] == true
            {
                received.insert("settings");
            }
            if response["id"] == "pomodoro"
                && response["payload"]["data"]["pomodoroAction"]["today"]["isRunning"] == true
            {
                received.insert("pomodoro");
            }
        }
    })
    .await
    .unwrap();
    core.shutdown().await;
}

use super::*;
use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Message as ClientMessage, client::IntoClientRequest};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scanning_with_300_http_requests_and_page_changes_preserves_the_encrypted_socket() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let directory = tempfile::tempdir().unwrap();
        let server = crate::server::ContentServer::open(directory.path()).unwrap();
        let state = server.runtime_state();
        state.prefs.set(plain_rs::features::prefs::pref_keys::API_PERMISSIONS, vec!["WRITE_EXTERNAL_STORAGE"]).unwrap();
        let host = state.host.clone();
        let (generation, mut incoming) = host.connect();
        let replies = tokio::spawn(async move {
            while let Some(request) = incoming.recv().await {
                let result = match request["method"].as_str().unwrap() {
                    "systemCapabilities" => json!(["MEDIA_SCAN"]),
                    "systemPermissionFacts" => json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}),
                    "onlineClientsChanged" | "systemWebActivity" | "httpServerStatusChanged" => Value::Null,
                    method => panic!("unexpected host method {method}"),
                };
                host.reply(generation, json!({"id":request["id"],"result":result})).unwrap();
            }
        });
        let session = plain_rs::features::services::sessions::create(&state, "fixture".into()).unwrap();
        let client = session.client_id;
        let key = plain_libs::utils::base64::base64_decode(&session.token);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/graphql", axum::routing::get(upgrade).post(crate::http::graphql::call)).with_state(state.clone());
        let serving = tokio::spawn(async { axum::serve(listener, app).await.unwrap() });
        let mut request = format!("ws://{address}/graphql?cid={client}").into_client_request().unwrap();
        request.headers_mut().insert("sec-websocket-protocol", PROTOCOL.parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        let init = graphql::seal_init(&key, &client, &serde_json::to_vec(&Init {
            timestamp_millis: chrono::Utc::now().timestamp_millis(),
            client_nonce: "ab".repeat(32),
        }).unwrap()).unwrap();
        socket.send(ClientMessage::Binary(init.clone().into())).await.unwrap();
        let ClientMessage::Binary(ack) = socket.next().await.unwrap().unwrap() else { panic!("encrypted acknowledgement required") };
        graphql::open_ack(&key, &client, &init, &ack).unwrap();
        let mut cipher = graphql::Stream::new(&client, &init, &ack, Role::Client).unwrap();
        for (id, query) in [("scan", "subscription { mediaScanProgress { indexed state } }"), ("media", "subscription { mediaLibraryUpdated { types } }")] {
            let frame = cipher.encode(json!({"type":"subscribe","id":id,"payload":{"query":query}}).to_string().as_bytes(), &key).unwrap();
            socket.send(ClientMessage::Binary(frame.into())).await.unwrap();
        }
        let mut ready = 0;
        while ready < 2 {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else { panic!("socket closed during subscribe") };
            let message: Value = serde_json::from_slice(&cipher.decode(&frame, &key).unwrap()).unwrap();
            assert_eq!(message["type"], "next", "{message}");
            if message["payload"]["extensions"]["subscriptionReady"] == true { ready += 1; }
        }
        let scan = state.media_scans.begin(1000).unwrap();
        scan.record(|| Ok(())).unwrap();
        state.media_scans.publish_catalog_types([1, 3]);
        let mut first_scan = false;
        let mut first_media = false;
        while !first_scan || !first_media {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else { panic!("socket closed before scan results") };
            let message: Value = serde_json::from_slice(&cipher.decode(&frame, &key).unwrap()).unwrap();
            assert_eq!(message["type"], "next", "{message}");
            first_scan |= message["payload"]["data"]["mediaScanProgress"].is_object();
            first_media |= message["payload"]["data"]["mediaLibraryUpdated"].is_object();
        }
        fs_burst_and_page_cancellation(&server, &mut socket, &mut cipher, &key, &client).await;
        for index in 1..1000 {
            scan.record(|| Ok(())).unwrap();
            state.media_scans.publish_catalog_types([1, 3]);
            if index % 10 == 0 { tokio::task::yield_now().await; }
        }
        scan.finish(Ok(())).unwrap();
        for index in 0..300 {
            let id = format!("page-{index}");
            for message in [json!({"type":"subscribe","id":id,"payload":{"query":"subscription { contentChanged }"}}), json!({"type":"complete","id":id})] {
                let frame = cipher.encode(message.to_string().as_bytes(), &key).unwrap();
                socket.send(ClientMessage::Binary(frame.into())).await.unwrap();
            }
        }
        let frame = cipher.encode(br#"{"type":"ping","payload":{"afterPageChange":true}}"#, &key).unwrap();
        socket.send(ClientMessage::Binary(frame.into())).await.unwrap();
        loop {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else { panic!("socket closed during scan and page changes") };
            let message: Value = serde_json::from_slice(&cipher.decode(&frame, &key).unwrap()).unwrap();
            assert!(message["type"] != "error" || message["payload"][0]["message"] == "resync_required", "{message}");
            if message["type"] == "pong" {
                assert_eq!(message["payload"]["afterPageChange"], true);
                break;
            }
        }
        let result = server.native_graphql().unwrap().execute("{ scanProgress { indexed state } }".into()).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.data.into_json().unwrap()["scanProgress"], json!({"indexed":1000,"state":"IDLE"}));
        socket.close(None).await.unwrap();
        serving.abort();
        replies.abort();
        server.shutdown().await;
    }).await.unwrap();
}

type TestSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn heartbeat(socket: &mut TestSocket, cipher: &mut graphql::Stream, key: &[u8]) {
    let frame = cipher
        .encode(br#"{"type":"ping","payload":{"fsResponsive":true}}"#, key)
        .unwrap();
    socket
        .send(ClientMessage::Binary(frame.into()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let ClientMessage::Binary(frame) = socket.next().await.unwrap().unwrap() else {
                panic!("WebSocket closed under fs load")
            };
            let message: Value =
                serde_json::from_slice(&cipher.decode(&frame, key).unwrap()).unwrap();
            if message["type"] == "pong" {
                assert_eq!(message["payload"]["fsResponsive"], true);
                break;
            }
        }
    })
    .await
    .unwrap();
}

async fn fs_burst_and_page_cancellation(
    server: &crate::server::ContentServer,
    socket: &mut TestSocket,
    cipher: &mut graphql::Stream,
    key: &[u8],
    client: &str,
) {
    use std::sync::atomic::Ordering;
    let state = server.runtime_state();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let pem = certificate.cert.pem();
    let (_, port) = server
        .start_public(
            0,
            0,
            pem.clone().into_bytes(),
            certificate.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
    let http = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(pem.as_bytes()).unwrap())
        .build()
        .unwrap();
    let origin = format!("https://localhost:{port}");
    let source = tempfile::tempdir().unwrap();
    let original = source.path().join("original.jpg");
    image::RgbImage::from_fn(800, 600, |x, y| {
        image::Rgb([(x % 220) as u8, (y % 220) as u8, 60])
    })
    .save(&original)
    .unwrap();
    let resource_key =
        plain_libs::utils::base64::base64_decode(&server.native_url_token().unwrap());
    let ids = (0..301)
        .map(|index| {
            let path = source.path().join(format!("photo-{index}.jpg"));
            std::fs::copy(&original, &path).unwrap();
            plain_rs::features::files::resource_link::seal(
                &plain_rs::features::files::resource_link::Scope::Global,
                &resource_key,
                &plain_rs::features::files::resource_link::Resource::reference(
                    path.to_str().unwrap(),
                ),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
    *state.thumbnails.decode_gate.lock().unwrap() = Some(gate.clone());
    let mut pending = tokio::task::JoinSet::new();
    for id in &ids[..300] {
        let request = http.get(format!("{origin}/fs")).query(&[
            ("id", id.as_str()),
            ("w", "512"),
            ("h", "512"),
        ]);
        pending.spawn(async move { request.send().await.unwrap() });
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        while state.thumbnails.admitted_requests() != 300
            || state.thumbnails.native_decodes.load(Ordering::SeqCst) != 4
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        http.get(format!("{origin}/health"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let plaintext = format!(
        "{}|{}|{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4(),
        json!({"query":r#"{noteCount(query:"")} "#})
    );
    let body = plain_libs::crypto::xchacha_encrypt_raw(key, plaintext.as_bytes()).unwrap();
    let response = http
        .post(format!("{origin}/graphql"))
        .header("c-id", client)
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.version(), reqwest::Version::HTTP_11);
    let context =
        plain_rs::infrastructure::transport::http_response::Context::new("/graphql", &body);
    let bytes = response.bytes().await.unwrap();
    let decoded = context.decrypt(key, &bytes).unwrap();
    let response: Value = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(response["data"]["noteCount"], 0);
    heartbeat(socket, cipher, key).await;
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 4);
    pending.abort_all();
    while pending.join_next().await.is_some() {}
    tokio::time::timeout(Duration::from_secs(10), async {
        while state.thumbnails.admitted_requests() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 4);
    *state.thumbnails.decode_gate.lock().unwrap() = None;
    gate.add_permits(4);
    let next = http
        .get(format!("{origin}/fs"))
        .query(&[("id", ids[300].as_str()), ("w", "512"), ("h", "512")])
        .send()
        .await
        .unwrap();
    assert_eq!(next.status(), StatusCode::OK);
    let image = image::load_from_memory(&next.bytes().await.unwrap()).unwrap();
    assert_eq!((image.width(), image.height()), (512, 512));
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 5);
    let mut complete = tokio::task::JoinSet::new();
    for id in &ids[..300] {
        let request = http.get(format!("{origin}/fs")).query(&[
            ("id", id.as_str()),
            ("w", "512"),
            ("h", "512"),
        ]);
        complete.spawn(async move {
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.version(), reqwest::Version::HTTP_11);
            let bytes = response.bytes().await.unwrap();
            let image = image::load_from_memory(&bytes).unwrap();
            assert_eq!((image.width(), image.height()), (512, 512));
        });
    }
    while let Some(result) = complete.join_next().await {
        result.unwrap();
    }
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 301);
    heartbeat(socket, cipher, key).await;
    let requests = (0..300)
        .map(|_| {
            let plaintext = format!(
                "{}|{}|{}",
                chrono::Utc::now().timestamp_millis(),
                uuid::Uuid::new_v4(),
                json!({"query":r#"{noteCount(query:"")}"#})
            );
            plain_libs::crypto::xchacha_encrypt_raw(key, plaintext.as_bytes()).unwrap()
        })
        .collect::<Vec<_>>();
    let configuration = source.path().join("http2.json");
    std::fs::write(&configuration,serde_json::to_vec(&json!({"origin":origin,"certificate":pem,"client":client,"requests":requests.iter().map(|body|plain_libs::utils::base64::base64_encode(body)).collect::<Vec<_>>()})).unwrap()).unwrap();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/plain-server/fixtures/http2-requests.mjs"
            ))
            .arg(configuration)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(responses.len(), 300);
    for (response, request) in responses.iter().zip(&requests) {
        assert_eq!(response["status"], 200);
        let bytes = plain_libs::utils::base64::base64_decode(response["body"].as_str().unwrap());
        let decoded =
            plain_rs::infrastructure::transport::http_response::Context::new("/graphql", request)
                .decrypt(key, &bytes)
                .unwrap();
        let result: Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(result["data"]["noteCount"], 0);
        assert!(result.get("errors").is_none());
    }
}

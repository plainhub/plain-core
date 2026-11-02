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
                    "onlineClientsChanged" | "systemWebActivity" => Value::Null,
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
        let http = reqwest::Client::new();
        let requests = (0..300).map(|_| {
            let request = format!("{}|{}|{}", chrono::Utc::now().timestamp_millis(), uuid::Uuid::new_v4(), json!({"query":"{ noteCount(query:\"\") }"}));
            let body = plain_libs::crypto::xchacha_encrypt_raw(&key, request.as_bytes()).unwrap();
            http.post(format!("http://{address}/graphql")).header("c-id", &client).body(body).send()
        }).collect::<Vec<_>>();
        let http_requests = tokio::spawn(async move { futures_util::future::join_all(requests).await });
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
        for response in http_requests.await.unwrap() { assert_eq!(response.unwrap().status(), StatusCode::OK); }
        let result = server.native_graphql().unwrap().execute("{ scanProgress { indexed state } }".into()).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.data.into_json().unwrap()["scanProgress"], json!({"indexed":1000,"state":"IDLE"}));
        socket.close(None).await.unwrap();
        serving.abort();
        replies.abort();
        server.shutdown().await;
    }).await.unwrap();
}

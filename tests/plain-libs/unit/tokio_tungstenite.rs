use super::*;
use futures_util::{SinkExt, StreamExt};
#[tokio::test]
async fn fragmented_transport_keeps_handshake_messages_ping_and_close() {
    let (client, server) = tokio::io::duplex(64);
    let server = tokio::spawn(async move {
        let mut socket = accept_async(server).await.unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap().into_text().unwrap(),
            "hello"
        );
        socket
            .send(crate::tungstenite::Message::binary(vec![1, 2, 3]))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
    });
    let (mut client, _) = client_async("ws://example.test/ws", client).await.unwrap();
    client
        .send(crate::tungstenite::Message::text("hello"))
        .await
        .unwrap();
    assert_eq!(
        client.next().await.unwrap().unwrap().into_data(),
        vec![1, 2, 3]
    );
    assert!(matches!(
        client.next().await.unwrap().unwrap(),
        crate::tungstenite::Message::Close(_)
    ));
    server.await.unwrap();
}

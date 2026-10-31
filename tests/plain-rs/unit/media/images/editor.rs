#[tokio::test]
async fn binary_updates_have_bounded_queue_and_exact_payload() {
    let (sender, mut receiver) = tokio::sync::broadcast::channel(64);
    let updates = super::Updates::new(sender);
    updates.publish("画布", "AQID").unwrap();
    let event = receiver.recv().await.unwrap();
    assert_eq!(event.event_type, "IMAGE_EDITOR_UPDATE");
    assert_eq!(
        event.binary_payload.unwrap(),
        [vec![6], "画布".as_bytes().to_vec(), vec![1, 2, 3]].concat()
    );
    for (id, state) in [("", "AQID"), ("p", "!"), (&"p".repeat(256), "AQID")] {
        assert!(updates.publish(id, state).is_err());
    }
    let data = crate::utils::base64::base64_encode(&vec![0; 8 * 1024 * 1024]);
    for _ in 0..3 {
        updates.publish("p", &data).unwrap();
    }
    assert!(updates.publish("p", &data).is_err());
    drop(receiver.recv().await.unwrap());
    updates.publish("p", &data).unwrap();
}

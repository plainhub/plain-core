use super::*;
use serde_json::json;

#[test]
fn queue_projection_preserves_large_progress_and_normalizes_attachment_defaults() {
    let peer = plain_rs::db::DPeer::new(
        "peer",
        "Peer",
        "127.0.0.1",
        Some(8443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    let queue = snapshot(json!({"runtimeId":"fixture","revision":5000000000_i64,"tasks":[{
        "id":"file","messageId":"message","generation":"generation","status":"PAUSED","error":"",
        "downloaded":6000000000_i64,"total":7000000000_i64,"speed":1000000000_i64,"peer":peer,
        "file":{"id":"file","uri":"fsid:fixture","size":7000000000_i64,"duration":2}
    }]}), &std::collections::HashSet::from(["peer"])).unwrap();
    assert_eq!(queue.revision, Long(5000000000));
    assert_eq!(queue.tasks[0].downloaded, Long(6000000000));
    assert_eq!(queue.tasks[0].file.duration_ms, Long(2000));
    assert_eq!(queue.tasks[0].file.file_name, "");
    assert_eq!(queue.tasks[0].peer.id, "peer");
    assert!(queue.tasks[0].peer.online);
}
#[test]
fn explicit_millisecond_duration_takes_precedence_and_legacy_overflow_is_rejected() {
    let file: FileReceipt = serde_json::from_value(
        json!({"id":"file","uri":"fsid:fixture","size":1,"durationMs":1234,"duration":2}),
    )
    .unwrap();
    assert_eq!(
        ChatDownloadFile::try_from(file).unwrap().duration_ms,
        Long(1234)
    );
    let file: FileReceipt = serde_json::from_value(
        json!({"id":"file","uri":"fsid:fixture","size":1,"legacyDurationSec":i64::MAX}),
    )
    .unwrap();
    assert!(ChatDownloadFile::try_from(file).is_err());
}

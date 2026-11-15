use super::*;

#[test]
fn first_partition_sata() {
    assert_eq!(first_partition_path("/dev/sda"), "/dev/sda1");
    assert_eq!(first_partition_path("/dev/sdb"), "/dev/sdb1");
}

#[test]
fn first_partition_nvme() {
    assert_eq!(first_partition_path("/dev/nvme0n1"), "/dev/nvme0n1p1");
    assert_eq!(first_partition_path("/dev/nvme1n2"), "/dev/nvme1n2p1");
}

#[test]
fn first_partition_mmc() {
    assert_eq!(first_partition_path("/dev/mmcblk0"), "/dev/mmcblk0p1");
}

#[test]
fn first_partition_empty() {
    assert_eq!(first_partition_path(""), "");
}

#[test]
fn require_tool_missing_returns_err() {
    assert!(require_tool("__definitely_not_a_real_tool_xyz__").is_err());
}

#[tokio::test]
async fn unmount_records_only_successful_effects_with_saved_volume_and_actor_facts() {
    use crate::features::audit_events::{EventData, EventType, EventVolume};
    use crate::features::services::audit_events::AuditEvents;
    use crate::infrastructure::db::{AuditEventClient, Db};
    use crate::infrastructure::platform::linux::mounts::automount::{EventSink, MountEvent};
    use std::sync::Arc;
    let db = Arc::new(Db::open(Path::new(":memory:")).unwrap());
    let writer = AuditEvents::start(db.clone()).unwrap();
    let sink_writer = writer.clone();
    let events: EventSink = Arc::new(move |event| match event {
        MountEvent::Audit(event) => sink_writer
            .record(
                event,
                Some(AuditEventClient {
                    client_id: "706c6169-8833-840e-a693-365203ce72a0".into(),
                    client_name: "Operator".into(),
                    ..Default::default()
                }),
            )
            .map_err(plain_libs::anyhow::Error::msg),
        MountEvent::Changed => panic!("unmount does not refresh by itself"),
    });
    let parent = EventVolume {
        id: Some("parent".into()),
        name: Some("Parent".into()),
        device_path: Some("/dev/generated1".into()),
        mount_path: "/mnt/generated".into(),
        file_system: Some("ext4".into()),
    };
    let child = EventVolume {
        id: Some("child".into()),
        name: Some("Child".into()),
        mount_path: "/mnt/generated/nested".into(),
        ..parent.clone()
    };
    let mut calls = Vec::new();
    let result = unmount_volumes(
        vec![parent.clone(), child.clone(), child.clone()],
        |path| {
            calls.push(path.to_owned());
            if path == parent.mount_path {
                Err(anyhow!("generated unmount failure"))
            } else {
                Ok(())
            }
        },
        &events,
    );
    assert!(result.is_err());
    assert_eq!(calls, [child.mount_path.clone(), parent.mount_path.clone()]);
    writer.flush().await.unwrap();
    let rows = db.audit_events(0, 10, None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].r#type, EventType::Unmount);
    assert_eq!(rows[0].client_id.as_deref(), Some("706c6169-8833-840e-a693-365203ce72a0"));
    assert!(matches!(&rows[0].data, EventData::Volume(data) if data.volume == child));
    writer.shutdown().await.unwrap();
}

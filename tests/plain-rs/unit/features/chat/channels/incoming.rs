use super::*;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::{SaveMode, peers};
use plain_libs::utils::base64::base64_encode;

fn db() -> Db {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut owner = DPeer::new(
        "owner",
        "Owner",
        "",
        None,
        crate::features::chat::enums::DeviceType::Computer,
    );
    owner.public_key = signing_public_key("owner");
    peers::save(&db, &[owner], SaveMode::Insert).unwrap();
    db
}

fn signing_key() -> [u8; 64] {
    plain_libs::crypto::ed25519_keypair_from_seed(&[9; 32])
}

fn signing_public_key(peer_id: &str) -> String {
    if peer_id == "owner" {
        base64_encode(&signing_key()[32..])
    } else {
        base64_encode(&[8; 32])
    }
}

fn sign(state: &mut State) {
    state.signature = plain_libs::crypto::ed25519_sign(
        &signing_key(), &state.signing_bytes("channel"),
    );
}

fn member(peer_id: &str, status: MemberStatus) -> MemberSnapshot {
    MemberSnapshot {
        peer_id: peer_id.into(),
        status,
        name: None,
        device_type: None,
        ip: None,
        signature_public_key: signing_public_key(peer_id),
        encryption_public_key: base64_encode(&[40; 32]),
        port: None,
    }
}

fn state(version: i64) -> State {
    let mut state = State {
        name: "Channel".into(),
        version,
        members: vec![
            member("owner", MemberStatus::Joined),
            member("guest", MemberStatus::Pending),
        ],
        signature: String::new(),
    };
    sign(&mut state);
    state
}

fn invite(version: i64) -> Notification {
    Notification::Invite(state(version))
}

fn stored(db: &Db) -> Option<DChannel> {
    crate::infrastructure::db::chat_store::channels::get(db, "channel").unwrap()
}

#[test]
fn invite_validates_roster_and_existing_owner_and_reinvites_only_at_new_version() {
    let db = db();
    assert!(receive(&db, "other", "owner", "channel", invite(1)).is_err());
    assert!(stored(&db).is_none());
    let mut forged = state(1);
    forged.members[0].signature_public_key = base64_encode(&[8; 32]);
    assert!(
        receive(
            &db,
            "guest",
            "owner",
            "channel",
            Notification::Invite(forged)
        )
        .is_err()
    );
    let first = receive(&db, "guest", "owner", "channel", invite(1)).unwrap();
    assert!(first.changed);
    assert_eq!(first.invite.unwrap().owner_peer_id, "owner");
    assert!(
        !receive(&db, "guest", "owner", "channel", invite(1))
            .unwrap()
            .changed
    );
    assert!(
        receive(
            &db,
            "guest",
            "other",
            "channel",
            Notification::Deleted { version: 2 }
        )
        .is_err()
    );
    assert!(
        receive(
            &db,
            "guest",
            "owner",
            "channel",
            Notification::MemberRemoved {
                removed_peer_id: "other".into(),
                version: 2
            }
        )
        .is_err()
    );
    let result = receive(
        &db,
        "guest",
        "owner",
        "channel",
        Notification::MemberRemoved {
            removed_peer_id: "guest".into(),
            version: 2,
        },
    )
    .unwrap();
    assert!(result.cancel.is_some());
    assert_eq!(stored(&db).unwrap().status, ChannelStatus::Kicked);
    assert!(receive(&db, "guest", "owner", "channel", invite(2)).is_err());
    assert!(
        receive(&db, "guest", "owner", "channel", invite(3))
            .unwrap()
            .changed
    );
}

#[test]
fn snapshots_validate_before_effects_and_roll_back_peer_creation_on_channel_write_failure() {
    let db = db();
    receive(&db, "guest", "owner", "channel", invite(1)).unwrap();
    let before = stored(&db).unwrap();
    assert!(
        receive(
            &db,
            "guest",
            "other",
            "channel",
            Notification::StateChanged(state(2))
        )
        .is_err()
    );
    assert!(
        receive(
            &db,
            "guest",
            "owner",
            "channel",
            Notification::StateChanged(state(-1))
        )
        .is_err()
    );
    assert!(
        !receive(
            &db,
            "guest",
            "owner",
            "channel",
            Notification::StateChanged(state(1))
        )
        .unwrap()
        .changed
    );
    let mut duplicate = state(2);
    duplicate
        .members
        .push(member("guest", MemberStatus::Joined));
    assert!(
        receive(
            &db,
            "guest",
            "owner",
            "channel",
            Notification::StateChanged(duplicate)
        )
        .is_err()
    );
    let update = || {
        let mut snapshot = state(2);
        snapshot.members.push(MemberSnapshot {
            name: Some("New peer".into()),
            ..member("new", MemberStatus::Joined)
        });
        snapshot
            .members
            .push(member("no-metadata", MemberStatus::Pending));
        sign(&mut snapshot);
        Notification::StateChanged(snapshot)
    };
    db.with_conn(|c| c.execute_batch("CREATE TRIGGER reject_channel BEFORE UPDATE ON chat_channels BEGIN SELECT RAISE(ABORT, 'test failure'); END;")).unwrap();
    assert!(receive(&db, "guest", "owner", "channel", update()).is_err());
    assert_eq!(stored(&db).unwrap(), before);
    assert!(peers::get(&db, "new").unwrap().is_none());
    db.with_conn(|c| c.execute_batch("DROP TRIGGER reject_channel;"))
        .unwrap();
    assert!(
        receive(&db, "guest", "owner", "channel", update())
            .unwrap()
            .changed
    );
    assert_eq!(peers::get(&db, "new").unwrap().unwrap().name, "New peer");
    assert!(peers::get(&db, "no-metadata").unwrap().unwrap().encryption_public_key.is_some());
    assert!(stored(&db).unwrap().members.contains("no-metadata"));
}

#[test]
fn invitation_replies_and_leave_affect_only_the_sender() {
    let db = db();
    let mut channel = DChannel::new("Channel", "owner");
    channel.id = "channel".into();
    channel.members = plain_libs::serde_json::to_string(&[
        ChannelMember::new("owner"),
        ChannelMember::pending("guest"),
        ChannelMember::pending("declining"),
    ])
    .unwrap();
    crate::infrastructure::db::chat_store::channels::save(&db, &[channel], SaveMode::Insert)
        .unwrap();
    assert!(
        !receive(
            &db,
            "owner",
            "stranger",
            "channel",
            Notification::InviteResponse { accept: true }
        )
        .unwrap()
        .changed
    );
    assert!(
        receive(
            &db,
            "owner",
            "declining",
            "channel",
            Notification::InviteResponse { accept: false }
        )
        .unwrap()
        .changed
    );
    assert!(
        !receive(
            &db,
            "owner",
            "declining",
            "channel",
            Notification::InviteResponse { accept: false }
        )
        .unwrap()
        .changed
    );
    assert!(
        receive(
            &db,
            "owner",
            "guest",
            "channel",
            Notification::InviteResponse { accept: true }
        )
        .unwrap()
        .broadcast
    );
    assert!(
        !receive(
            &db,
            "owner",
            "guest",
            "channel",
            Notification::InviteResponse { accept: false }
        )
        .unwrap()
        .changed
    );
    assert!(
        receive(&db, "owner", "guest", "channel", Notification::MemberLeft)
            .unwrap()
            .broadcast
    );
    assert_eq!(
        decode_members(&stored(&db).unwrap().members),
        vec![ChannelMember::new("owner")]
    );
}

#[test]
fn a_late_unavailable_reply_preserves_the_newer_channel() {
    let db = db();
    receive(&db, "guest", "owner", "channel", invite(1)).unwrap();
    let before = stored(&db).unwrap();
    receive(&db, "guest", "owner", "channel", Notification::StateChanged(state(2))).unwrap();
    let current = stored(&db).unwrap();
    let result = receive(&db, "guest", "owner", "channel", Notification::Unavailable {
        version: before.version, updated_at: before.updated_at,
    }).unwrap();
    assert!(!result.changed);
    assert_eq!(stored(&db).unwrap(), current);
}

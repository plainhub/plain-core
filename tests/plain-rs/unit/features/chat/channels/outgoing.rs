use super::*;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::{SaveMode, peers};
use crate::infrastructure::transport::graphql_request::SignedEnvelope;

#[test]
fn channel_snapshots_include_each_member_once_and_sign_typed_variables() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let (keys, public) = plain_libs::crypto::ed25519_generate();
    let mut peer = DPeer::new(
        "paired",
        "Paired member",
        "",
        Some(1234),
        DeviceType::Unknown,
    );
    peer.public_key = base64_encode(&[8; 32]);
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    let mut channel = DChannel::new("Group", "owner");
    channel.key = base64_encode(&[7; 32]);
    let state = snapshot(
        &db,
        &channel,
        "owner",
        "Owner".into(),
        DeviceType::Computer,
        &public,
        vec![
            ChannelMember::new("owner"),
            ChannelMember::pending("paired"),
            ChannelMember::new("missing"),
        ],
    )
    .unwrap();
    let envelope: SignedEnvelope = plain_libs::serde_json::from_str(
        &Action::Invite
            .request(&channel, "paired", Some(&state))
            .unwrap()
            .sign(&keys, "")
            .unwrap(),
    )
    .unwrap();
    assert!(plain_libs::crypto::ed25519_verify(
        &base64_encode(&public),
        &envelope.signing_bytes(""),
        &envelope.signature
    ));
    assert!(
        envelope
            .query
            .contains("notifyChannelInvite(channelId: $channelId, input: $input)")
    );
    let vars = Value::Object(envelope.variables);
    assert_eq!(
        vars,
        json!({
            "channelId":channel.id,
            "input":{"channelKey":channel.key,"state":{"name":"Group","version":channel.version,"members":[
                {"peerId":"owner","status":"JOINED","name":"Owner","deviceType":"COMPUTER","publicKey":base64_encode(&public)},
                {"peerId":"paired","status":"PENDING","name":"Paired member","deviceType":"OTHER","ip":"","publicKey":base64_encode(&[8;32]),"port":1234},
                {"peerId":"missing","status":"JOINED"}
            ]}}
        })
    );
}

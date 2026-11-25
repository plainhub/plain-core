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
    peer.encryption_public_key = Some([40; 32]);
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    let channel = DChannel::new("Group", "owner");
    let state = snapshot(
        &db,
        &channel,
        "owner",
        "Owner".into(),
        DeviceType::Computer,
        &keys,
        &[41; 32],
        vec![
            ChannelMember::new("owner"),
            ChannelMember::pending("paired"),
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
        &envelope.signing_bytes("", None),
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
            "input":{"name":"Group","version":channel.version,"signature":state.signature,"members":[
                {"peerId":"owner","status":"JOINED","name":"Owner","deviceType":"COMPUTER","signaturePublicKey":base64_encode(&public),"encryptionPublicKey":base64_encode(&[41;32])},
                {"peerId":"paired","status":"PENDING","name":"Paired member","deviceType":"OTHER","ip":"","signaturePublicKey":base64_encode(&[8;32]),"encryptionPublicKey":base64_encode(&[40;32]),"port":1234}
            ]}
        })
    );
}

use super::*;
use plain_libs::{
    crypto::{ed25519_generate, ed25519_sign, ed25519_verify},
    serde_json::json,
    utils::base64::base64_encode,
};

#[test]
fn signature_purposes_are_distinct_and_not_interchangeable() {
    let purposes = [
        SignaturePurpose::AuthServerHello,
        SignaturePurpose::PairRequest,
        SignaturePurpose::PairingAccept,
        SignaturePurpose::PairingReject,
        SignaturePurpose::PairCancel,
        SignaturePurpose::PeerRequest,
        SignaturePurpose::PeerStatus,
        SignaturePurpose::QrPairAccept,
        SignaturePurpose::ChannelMembership,
        SignaturePurpose::PeerTlsCertificate,
    ];
    let (key, public) = ed25519_generate();
    let public = base64_encode(&public);
    for (i, purpose) in purposes.iter().enumerate() {
        let payload = json!([purpose, "same-content"]).to_string();
        let signature = ed25519_sign(&key, payload.as_bytes());
        for (j, other) in purposes.iter().enumerate() {
            let other = json!([other, "same-content"]).to_string();
            assert_eq!(
                ed25519_verify(&public, other.as_bytes(), &signature),
                i == j
            );
        }
    }
}

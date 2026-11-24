use crate::features::peers::pairing::protocol::PairingRequest;
use crate::features::peers::pairing::protocol::PairingAccept;
use crate::features::peers::pairing::timestamp_ok;
pub fn login_chat_key(shared: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::new().chain_update(b"chat-pairing").chain_update(shared).finalize();
    plain_libs::utils::base64::base64_encode(&hash)
}

pub(crate) fn same_public_key(left: &str, right: &str) -> bool {
    let decode = plain_libs::utils::base64::base64_decode_checked;
    matches!((decode(left), decode(right)), (Ok(left), Ok(right)) if left.len() == 32 && left == right)
}

pub fn verify_request(request: &PairingRequest) -> bool {
    !request.from_id.is_empty()
        && !request.to_id.is_empty()
        && request.to_id != request.from_id
        && plain_libs::utils::base64::base64_decode(&request.target_public_key).len() == 32
        && super::peer_store::encryption_key(&request.payload.encryption_public_key).is_ok()
        && timestamp_ok(request.payload.timestamp)
        && plain_libs::crypto::ed25519_verify(
            &request.payload.signature_public_key,
            request.signature_data().as_bytes(),
            &request.payload.signature,
        )
}
pub fn verify_accept(response: &PairingAccept, public_key: &str) -> bool {
    super::peer_store::encryption_key(&response.encryption_public_key).is_ok()
        && plain_libs::crypto::ed25519_verify(
        public_key,
        response.signature_data().as_bytes(),
        &response.signature,
    )
}

pub fn verify_cancel(
    cancel: &crate::features::peers::pairing::protocol::PairingCancel,
    actor: &str,
) -> bool {
    cancel.to_id == actor
        && !actor.is_empty()
        && !cancel.from_id.is_empty()
        && !cancel.request_signature.is_empty()
        && timestamp_ok(cancel.timestamp)
        && plain_libs::crypto::ed25519_verify(
            &cancel.signature_public_key,
            cancel.signature_data().as_bytes(),
            &cancel.signature,
        )
}

pub fn verify_reject(reply: &crate::features::peers::pairing::protocol::PairingReject, public_key: &str) -> bool {
    plain_libs::crypto::ed25519_verify(public_key, reply.signature_data().as_bytes(), &reply.signature)
}

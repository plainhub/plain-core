use crate::features::peers::pairing::protocol::PairingRequest;
use crate::features::peers::pairing::protocol::PairingResponse;
use crate::features::peers::pairing::timestamp_ok;
pub fn verify_request(request: &PairingRequest) -> bool {
    !request.from_id.is_empty()
        && !request.to_id.is_empty()
        && request.to_id != request.from_id
        && plain_libs::utils::base64::base64_decode(&request.target_public_key).len() == 32
        && timestamp_ok(request.payload.timestamp)
        && plain_libs::crypto::ed25519_verify(
            &request.payload.signature_public_key,
            request.signature_data().as_bytes(),
            &request.payload.signature,
        )
}
pub fn verify_response(response: &PairingResponse, public_key: &str) -> bool {
    plain_libs::crypto::ed25519_verify(
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

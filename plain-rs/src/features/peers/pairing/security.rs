use crate::features::peers::pairing::protocol::PairingRequest;
use crate::features::peers::pairing::protocol::PairingResponse;
use crate::features::peers::pairing::timestamp_ok;
pub fn verify_request(request: &PairingRequest) -> bool {
    !request.from_id.is_empty()
        && timestamp_ok(request.timestamp)
        && crate::libs::crypto::ed25519_verify(
            &request.signature_public_key,
            request.signature_data().as_bytes(),
            &request.signature,
        )
}
pub fn verify_response(response: &PairingResponse, actor: &str, expected: &str) -> bool {
    !response.request_signature.is_empty()
        && response.to_id == actor
        && response.from_id == expected
        && !actor.is_empty()
        && !expected.is_empty()
        && timestamp_ok(response.timestamp)
        && crate::libs::crypto::ed25519_verify(
            &response.signature_public_key,
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
        && crate::libs::crypto::ed25519_verify(
            &cancel.signature_public_key,
            cancel.signature_data().as_bytes(),
            &cancel.signature,
        )
}

use super::graphql_request::SignedEnvelope;
use super::http_request;
use plain_libs::crypto::hpke;
use plain_libs::serde_json;

const RESPONSE: &[u8] = b"peer-graphql-response";

#[cfg(feature = "prefs")]
pub(crate) fn open(key: &[u8; 32], channel: &str, body: &[u8]) -> Option<(Vec<u8>, [u8; 32])> {
    let (enc, ciphertext) = body.split_first_chunk::<32>()?;
    let context = hpke::setup_recipient(key, enc, &http_request::aad("POST", "/peer_graphql"))?;
    let response_key = *context.export(RESPONSE);
    Some((context.open(channel.as_bytes(), ciphertext)?, response_key))
}

pub(crate) fn seal(
    key: &[u8; 32],
    channel: &str,
    sender: &str,
    recipient: &str,
    message_id: Option<&str>,
    signing_key: &[u8],
    body: &[u8],
) -> Option<(Vec<u8>, [u8; 32])> {
    let mut request: SignedEnvelope = serde_json::from_slice(body).ok()?;
    request.message_id = message_id.map(ToOwned::to_owned);
    request.sign(signing_key, channel, Some((sender, recipient))).ok()?;
    let plaintext = serde_json::to_vec(&request).ok()?;
    let (enc, context) = hpke::setup_sender(key, &http_request::aad("POST", "/peer_graphql"))?;
    let response_key = *context.export(RESPONSE);
    let ciphertext = context.seal(channel.as_bytes(), &plaintext)?;
    let mut wire = Vec::with_capacity(enc.len() + ciphertext.len());
    wire.extend_from_slice(&enc);
    wire.extend_from_slice(&ciphertext);
    Some((wire, response_key))
}

use super::http_request;
use plain_libs::crypto::hpke;

const RESPONSE: &[u8] = b"peer-graphql-response";

pub(crate) fn open(key: &[u8; 32], channel: &str, body: &[u8]) -> Option<(Vec<u8>, [u8; 32])> {
    let (enc, ciphertext) = body.split_first_chunk::<32>()?;
    let context = hpke::setup_recipient(key, enc, &http_request::aad("POST", "/peer_graphql"))?;
    let response_key = *context.export(RESPONSE);
    Some((context.open(channel.as_bytes(), ciphertext)?, response_key))
}

pub(crate) fn seal(key: &[u8; 32], channel: &str, plaintext: &[u8]) -> Option<(Vec<u8>, [u8; 32])> {
    let (enc, context) = hpke::setup_sender(key, &http_request::aad("POST", "/peer_graphql"))?;
    let response_key = *context.export(RESPONSE);
    let ciphertext = context.seal(channel.as_bytes(), plaintext)?;
    let mut wire = Vec::with_capacity(enc.len() + ciphertext.len());
    wire.extend_from_slice(&enc);
    wire.extend_from_slice(&ciphertext);
    Some((wire, response_key))
}

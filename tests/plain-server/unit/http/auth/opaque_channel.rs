use super::*;
fn channel(context: &[u8]) -> LoginChannel {
    LoginChannel::Opaque(OpaqueChannel {
        key: [7; 32],
        context: context.to_vec(),
        credential: plain_rs::features::prefs::security::PasswordCredential {
            setup: String::new(),
            record: String::new(),
        },
        created: Instant::now(),
    })
}
#[test]
fn opaque_metadata_is_bound_to_context_and_direction() {
    let context = plain_libs::auth::context("client", &[3; 32]);
    let request = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
        &[7; 32],
        b"metadata",
        &plain_libs::auth::auth_aad(&context, false),
    )
    .unwrap();
    let c = channel(&context);
    assert_eq!(c.decrypt_request(&request, now_ms()).unwrap(), b"metadata");
    assert!(
        channel(&plain_libs::auth::context("other", &[3; 32]))
            .decrypt_request(&request, now_ms())
            .is_err()
    );
    let response = c.encrypt_response(b"result", now_ms()).unwrap();
    assert!(c.decrypt_request(&response, now_ms()).is_err());
    assert_eq!(
        plain_libs::crypto::xchacha_decrypt_raw_with_aad(
            &[7; 32],
            &response,
            &plain_libs::auth::auth_aad(&context, true)
        )
        .unwrap(),
        b"result"
    );
    let mut damaged = request;
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    assert!(c.decrypt_request(&damaged, now_ms()).is_err());
}
#[test]
fn opaque_channel_expires() {
    let mut c = channel(b"context");
    let LoginChannel::Opaque(inner) = &mut c else {
        unreachable!()
    };
    inner.created = Instant::now() - Duration::from_secs(301);
    assert!(c.encrypt_response(b"result", now_ms()).is_err());
    assert!(c.decrypt_request(b"request", now_ms()).is_err());
}

use super::Receipt;
use super::claim;
use super::digest;
use super::permission_allowed;

#[test]
fn sms_send_requires_explicit_api_permission() {
    assert!(!permission_allowed(&[]));
    assert!(!permission_allowed(&["READ_SMS".into()]));
    assert!(permission_allowed(&["SEND_SMS".into()]));
}

#[test]
fn request_id_claim_deduplicates_matching_payload_and_rejects_reuse() {
    let mut receipts = Vec::<Receipt>::new();
    assert!(claim(&mut receipts, "client-request", "payload-a", 100).unwrap());
    assert!(!claim(&mut receipts, "client-request", "payload-a", 101).unwrap());
    assert!(claim(&mut receipts, "client-request", "payload-b", 102).is_err());
}

#[test]
fn idempotency_digests_do_not_store_sms_content() {
    assert_ne!(digest(b"number\0request"), digest(b"number\0other"));
    assert_ne!(digest(b"body-a"), digest(b"body-b"));
}

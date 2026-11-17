use super::*;

#[test]
fn failure_codes_preserve_only_distinct_pairing_outcomes() {
    use PairingErrorCode as Public;
    for (cause, expected) in [
        (AuthErrorCode::LoginExpired, Some(Public::Timeout)),
        (AuthErrorCode::Timeout, Some(Public::Timeout)),
        (AuthErrorCode::Rejected, Some(Public::Rejected)),
        (AuthErrorCode::InvalidPassword, Some(Public::InvalidPassword)),
        (AuthErrorCode::PeerIdentityChanged, Some(Public::PeerIdentityChanged)),
        (AuthErrorCode::TooManyLoginAttempts, Some(Public::TooManyLoginAttempts)),
        (AuthErrorCode::TooManyLoginConnections, Some(Public::InternalError)),
        (AuthErrorCode::PasswordSetupRequired, Some(Public::InternalError)),
        (AuthErrorCode::Canceled, None),
    ] {
        let error = plain_libs::anyhow::Error::new(cause).context("auth failed");
        assert_eq!(public_failure(&error), expected);
        assert_eq!(AuthErrorCode::from_error(&error), Some(cause));
    }
    for (cause, expected) in [
        (PairingFailure::PasswordRequired, Some(Public::PasswordRequired)),
        (PairingFailure::FailedConnectWs, Some(Public::InternalError)),
        (PairingFailure::Canceled, None),
    ] {
        assert_eq!(public_failure(&cause.into()), expected);
    }
}

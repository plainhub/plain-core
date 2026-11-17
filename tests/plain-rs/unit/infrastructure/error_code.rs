use super::*;

#[test]
fn error_codes_preserve_type_and_wire_value_with_context() {
    for (code, wire) in [
        (ErrorCode::InvalidPassword, "invalid_password"),
        (ErrorCode::PeerIdentityChanged, "peer_identity_changed"),
        (ErrorCode::RemoteAccessDisabled, "remote_access_disabled"),
        (ErrorCode::Rejected, "rejected"),
        (ErrorCode::LoginExpired, "login_expired"),
        (ErrorCode::TooManyLoginAttempts, "too_many_login_attempts"),
    ] {
        let error = plain_libs::anyhow::Error::new(code).context("request failed");
        assert_eq!(ErrorCode::from_error(&error), code);
        assert_eq!(code.as_str(), wire);
        assert_eq!(wire.parse::<ErrorCode>().unwrap(), code);
        assert_eq!(plain_libs::serde_json::to_value(code).unwrap(), wire);
    }
    assert!("private diagnostic".parse::<ErrorCode>().is_err());
    assert_eq!(
        ErrorCode::from_error(&plain_libs::anyhow::anyhow!("private diagnostic")),
        ErrorCode::InternalError
    );
}

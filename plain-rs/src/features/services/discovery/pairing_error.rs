use crate::features::services::auth_error::AuthErrorCode;

crate::feature_error! { PairingErrorCode {
    Timeout => "TIMEOUT",
    Rejected => "REJECTED",
    PasswordRequired => "PASSWORD_REQUIRED",
    InvalidPassword => "INVALID_PASSWORD",
    PeerIdentityChanged => "PEER_IDENTITY_CHANGED",
    TooManyLoginAttempts => "TOO_MANY_LOGIN_ATTEMPTS",
    InternalError => "INTERNAL_ERROR",
} }

#[derive(Debug)]
pub(super) enum PairingFailure {
    Canceled,
    Timeout,
    Rejected,
    BleConnectionClosing,
    BlePairingCapacityExceeded,
    FailedConnect,
    PairingTransportUnavailable,
    PairingAddressUnavailable,
    FailedConnectWs,
    InvalidAuthResponse,
    InvalidSession,
    PasswordRequired,
    PeerIdentityChanged,
    RemoteAccessDisabled,
    SignatureVerificationFailed,
    ChatPairingUnavailable,
}

impl std::fmt::Display for PairingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PairingFailure {}

pub(super) fn public_failure(error: &plain_libs::anyhow::Error) -> Option<PairingErrorCode> {
    use PairingErrorCode as Public;

    if let Some(cause) = error.chain().find_map(|cause| cause.downcast_ref::<PairingFailure>()) {
        return match cause {
            PairingFailure::Canceled => None,
            PairingFailure::Timeout => Some(Public::Timeout),
            PairingFailure::Rejected => Some(Public::Rejected),
            PairingFailure::PasswordRequired => Some(Public::PasswordRequired),
            PairingFailure::PeerIdentityChanged => Some(Public::PeerIdentityChanged),
            _ => Some(Public::InternalError),
        };
    }
    match AuthErrorCode::for_failure(error) {
        AuthErrorCode::Canceled => return None,
        AuthErrorCode::Timeout => return Some(Public::Timeout),
        AuthErrorCode::Rejected => return Some(Public::Rejected),
        AuthErrorCode::InvalidPassword => return Some(Public::InvalidPassword),
        AuthErrorCode::PeerIdentityChanged => return Some(Public::PeerIdentityChanged),
        AuthErrorCode::TooManyLoginAttempts => return Some(Public::TooManyLoginAttempts),
        _ => {}
    }
    #[cfg(feature = "prefs")]
    {
        use plain_libs::auth::client::LoginError;
        match error.chain().find_map(|cause| cause.downcast_ref::<LoginError>()) {
            Some(LoginError::Password) => return Some(Public::InvalidPassword),
            Some(LoginError::PeerIdentity) => return Some(Public::PeerIdentityChanged),
            _ => {}
        }
    }
    Some(Public::InternalError)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_error.rs"]
mod tests;

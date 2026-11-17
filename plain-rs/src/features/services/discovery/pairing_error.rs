crate::feature_error! { PairingErrorCode {
    Canceled => "CANCELED",
    Timeout => "TIMEOUT",
    InternalError => "INTERNAL_ERROR",
    Rejected => "REJECTED",
    BleConnectionClosing => "BLE_CONNECTION_CLOSING",
    BlePairingCapacityExceeded => "BLE_PAIRING_CAPACITY_EXCEEDED",
    FailedConnect => "FAILED_CONNECT",
    PairingTransportUnavailable => "PAIRING_TRANSPORT_UNAVAILABLE",
    PairingAddressUnavailable => "PAIRING_ADDRESS_UNAVAILABLE",
    FailedConnectWs => "FAILED_CONNECT_WS",
    InvalidAuthResponse => "INVALID_AUTH_RESPONSE",
    InvalidSession => "INVALID_SESSION",
    PasswordRequired => "PASSWORD_REQUIRED",
    PeerIdentityChanged => "PEER_IDENTITY_CHANGED",
    RemoteAccessDisabled => "REMOTE_ACCESS_DISABLED",
    SignatureVerificationFailed => "SIGNATURE_VERIFICATION_FAILED",
    ChatPairingUnavailable => "CHAT_PAIRING_UNAVAILABLE",
    InvalidPassword => "INVALID_PASSWORD",
    LoginExpired => "LOGIN_EXPIRED",
    TooManyLoginAttempts => "TOO_MANY_LOGIN_ATTEMPTS",
    PasswordSetupRequired => "PASSWORD_SETUP_REQUIRED",
    SecureLoginUnavailable => "SECURE_LOGIN_UNAVAILABLE",
    InvalidLoginRequest => "INVALID_LOGIN_REQUEST",
    TooManyLoginConnections => "TOO_MANY_LOGIN_CONNECTIONS",
} }

impl From<crate::features::services::auth_error::AuthErrorCode> for PairingErrorCode {
    fn from(code: crate::features::services::auth_error::AuthErrorCode) -> Self {
        use crate::features::services::auth_error::AuthErrorCode;
        match code {
            AuthErrorCode::Canceled => Self::Canceled,
            AuthErrorCode::EventStorageFailed => Self::InternalError,
            AuthErrorCode::InternalError => Self::InternalError,
            AuthErrorCode::InvalidAuthResponse => Self::InvalidAuthResponse,
            AuthErrorCode::InvalidIdentity => Self::InvalidAuthResponse,
            AuthErrorCode::InvalidLoginIdentity => Self::InvalidAuthResponse,
            AuthErrorCode::InvalidLoginKey => Self::InvalidAuthResponse,
            AuthErrorCode::InvalidLoginRequest => Self::InvalidLoginRequest,
            AuthErrorCode::InvalidPassword => Self::InvalidPassword,
            AuthErrorCode::LoginExpired => Self::LoginExpired,
            AuthErrorCode::PasswordSetupRequired => Self::PasswordSetupRequired,
            AuthErrorCode::PeerIdentityChanged => Self::PeerIdentityChanged,
            AuthErrorCode::Rejected => Self::Rejected,
            AuthErrorCode::RemoteAccessDisabled => Self::RemoteAccessDisabled,
            AuthErrorCode::TooManyLoginAttempts => Self::TooManyLoginAttempts,
            AuthErrorCode::TooManyLoginConnections => Self::TooManyLoginConnections,
            AuthErrorCode::Timeout => Self::Timeout,
            AuthErrorCode::SecureLoginUnavailable => Self::SecureLoginUnavailable,
            AuthErrorCode::SignatureVerificationFailed => Self::SignatureVerificationFailed,
            AuthErrorCode::FailedConnectWs => Self::FailedConnectWs,
            AuthErrorCode::ProxyAuthFailed => Self::FailedConnectWs,
            AuthErrorCode::Failed => Self::InternalError,
            AuthErrorCode::Unauthorized => Self::InvalidSession,
            AuthErrorCode::ChatPairingUnavailable => Self::ChatPairingUnavailable,
        }
    }
}

#[cfg(feature = "prefs")]
impl From<plain_libs::auth::client::LoginError> for PairingErrorCode {
    fn from(error: plain_libs::auth::client::LoginError) -> Self {
        use plain_libs::auth::client::LoginError;
        match error {
            LoginError::Randomness => Self::SecureLoginUnavailable,
            LoginError::Start | LoginError::SigningKey | LoginError::Response => Self::InvalidAuthResponse,
            LoginError::Password => Self::InvalidPassword,
            LoginError::PeerIdentity => Self::PeerIdentityChanged,
        }
    }
}

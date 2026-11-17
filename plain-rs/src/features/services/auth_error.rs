crate::feature_error! { AuthErrorCode {
    Canceled => "CANCELED",
    EventStorageFailed => "EVENT_STORAGE_FAILED",
    InternalError => "INTERNAL_ERROR",
    InvalidAuthResponse => "INVALID_AUTH_RESPONSE",
    InvalidIdentity => "INVALID_IDENTITY",
    InvalidLoginIdentity => "INVALID_LOGIN_IDENTITY",
    InvalidLoginKey => "INVALID_LOGIN_KEY",
    InvalidLoginRequest => "INVALID_LOGIN_REQUEST",
    InvalidPassword => "INVALID_PASSWORD",
    LoginExpired => "LOGIN_EXPIRED",
    PasswordSetupRequired => "PASSWORD_SETUP_REQUIRED",
    PeerIdentityChanged => "PEER_IDENTITY_CHANGED",
    Rejected => "REJECTED",
    RemoteAccessDisabled => "REMOTE_ACCESS_DISABLED",
    TooManyLoginAttempts => "TOO_MANY_LOGIN_ATTEMPTS",
    TooManyLoginConnections => "TOO_MANY_LOGIN_CONNECTIONS",
    Timeout => "TIMEOUT",
    SecureLoginUnavailable => "SECURE_LOGIN_UNAVAILABLE",
    SignatureVerificationFailed => "SIGNATURE_VERIFICATION_FAILED",
    FailedConnectWs => "FAILED_CONNECT_WS",
    ProxyAuthFailed => "PROXY_AUTH_FAILED",
    Failed => "FAILED",
    Unauthorized => "UNAUTHORIZED",
    ChatPairingUnavailable => "CHAT_PAIRING_UNAVAILABLE",
} }

#[cfg(feature = "prefs")]
impl From<plain_libs::auth::client::LoginError> for AuthErrorCode {
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

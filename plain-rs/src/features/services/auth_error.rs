crate::feature_error! { AuthErrorCode {
    Canceled => "CANCELED",
    InternalError => "INTERNAL_ERROR",
    InvalidLoginRequest => "INVALID_LOGIN_REQUEST",
    InvalidPassword => "INVALID_PASSWORD",
    PasswordSetupRequired => "PASSWORD_SETUP_REQUIRED",
    PeerIdentityChanged => "PEER_IDENTITY_CHANGED",
    Rejected => "REJECTED",
    RemoteAccessDisabled => "REMOTE_ACCESS_DISABLED",
    TooManyLoginAttempts => "TOO_MANY_LOGIN_ATTEMPTS",
    Timeout => "TIMEOUT",
} }

#[derive(Debug)]
pub enum AuthFailure {
    InvalidAuthResponse,
    InvalidIdentity,
    InvalidLoginIdentity,
    InvalidLoginKey,
    LoginExpired,
    SignatureVerificationFailed,
}

impl std::fmt::Display for AuthFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AuthFailure {}

impl AuthErrorCode {
    pub fn for_failure(error: &plain_libs::anyhow::Error) -> Self {
        if let Some(code) = Self::from_error(error) {
            return code;
        }
        if error.downcast_ref::<plain_libs::serde_json::Error>().is_some() {
            return Self::InvalidLoginRequest;
        }
        match error.chain().find_map(|cause| cause.downcast_ref::<AuthFailure>()) {
            Some(AuthFailure::InvalidLoginIdentity | AuthFailure::InvalidLoginKey) => Self::InvalidLoginRequest,
            Some(AuthFailure::LoginExpired) => Self::Timeout,
            _ => Self::InternalError,
        }
    }
}

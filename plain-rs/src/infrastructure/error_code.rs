use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum ErrorCode {
    EventStorageFailed,
    UnsupportedCapability,
    SlowClient,
    ActionNotFound,
    BulkQueryRequired,
    Canceled,
    ChatPairingUnavailable,
    ClipboardSyncDisabled,
    ConnectionTimeout,
    Failed,
    FailedConnect,
    FailedConnectWs,
    FeedNotFound,
    Forbidden,
    InternalError,
    InvalidAuthResponse,
    InvalidClipboardText,
    InvalidFeedUrl,
    InvalidIdentity,
    InvalidKeyValue,
    InvalidLoginIdentity,
    InvalidLoginKey,
    InvalidLoginRequest,
    InvalidPassword,
    InvalidPrefValue,
    InvalidSession,
    InvalidSubscription,
    LoginExpired,
    ManagedSocketClosed,
    ManagedSubscriptionClosed,
    NoPermission,
    PasswordRequired,
    PasswordSetupRequired,
    PeerIdentityChanged,
    PeerRemoved,
    ProxyAuthFailed,
    Rejected,
    RemoteAccessDisabled,
    SecureLoginUnavailable,
    SignatureVerificationFailed,
    SubscriptionResultTooLarge,
    Timeout,
    TooManyLoginAttempts,
    TooManyLoginConnections,
    Unauthorized,
    MountFailed,
    FormatDiskFailed,
    PairingTransportUnavailable,
    PairingAddressUnavailable,
    RequestContextChanged,
    PeerHttpsUnavailable,
    PeerAddressUnavailable,
    ArchiveEntryUnavailable,
    PreferenceUpdateFailed,
    BleConnectionClosing,
    BlePairingCapacityExceeded,
    ResyncRequired,
    NetworkError,
    NativeError,
    NativePeerIdMissing,
    InvalidRequest,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EventStorageFailed => "event_storage_failed",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::SlowClient => "slow_client",
            Self::ActionNotFound => "action_not_found",
            Self::BulkQueryRequired => "bulk_query_required",
            Self::Canceled => "canceled",
            Self::ChatPairingUnavailable => "chat_pairing_unavailable",
            Self::ClipboardSyncDisabled => "clipboard_sync_disabled",
            Self::ConnectionTimeout => "connection_timeout",
            Self::Failed => "failed",
            Self::FailedConnect => "failed_connect",
            Self::FailedConnectWs => "failed_connect_ws",
            Self::FeedNotFound => "feed_not_found",
            Self::Forbidden => "forbidden",
            Self::InternalError => "internal_error",
            Self::InvalidAuthResponse => "invalid_auth_response",
            Self::InvalidClipboardText => "invalid_clipboard_text",
            Self::InvalidFeedUrl => "invalid_feed_url",
            Self::InvalidIdentity => "invalid_identity",
            Self::InvalidKeyValue => "invalid_key_value",
            Self::InvalidLoginIdentity => "invalid_login_identity",
            Self::InvalidLoginKey => "invalid_login_key",
            Self::InvalidLoginRequest => "invalid_login_request",
            Self::InvalidPassword => "invalid_password",
            Self::InvalidPrefValue => "invalid_pref_value",
            Self::InvalidSession => "invalid_session",
            Self::InvalidSubscription => "invalid_subscription",
            Self::LoginExpired => "login_expired",
            Self::ManagedSocketClosed => "managed_socket_closed",
            Self::ManagedSubscriptionClosed => "managed_subscription_closed",
            Self::NoPermission => "no_permission",
            Self::PasswordRequired => "password_required",
            Self::PasswordSetupRequired => "password_setup_required",
            Self::PeerIdentityChanged => "peer_identity_changed",
            Self::PeerRemoved => "peer_removed",
            Self::ProxyAuthFailed => "proxy_auth_failed",
            Self::Rejected => "rejected",
            Self::RemoteAccessDisabled => "remote_access_disabled",
            Self::SecureLoginUnavailable => "secure_login_unavailable",
            Self::SignatureVerificationFailed => "signature_verification_failed",
            Self::SubscriptionResultTooLarge => "subscription_result_too_large",
            Self::Timeout => "timeout",
            Self::TooManyLoginAttempts => "too_many_login_attempts",
            Self::TooManyLoginConnections => "too_many_login_connections",
            Self::Unauthorized => "unauthorized",
            Self::MountFailed => "mount_failed",
            Self::FormatDiskFailed => "format_disk_failed",
            Self::PairingTransportUnavailable => "pairing_transport_unavailable",
            Self::PairingAddressUnavailable => "pairing_address_unavailable",
            Self::RequestContextChanged => "request_context_changed",
            Self::PeerHttpsUnavailable => "peer_https_unavailable",
            Self::PeerAddressUnavailable => "peer_address_unavailable",
            Self::ArchiveEntryUnavailable => "archive_entry_unavailable",
            Self::PreferenceUpdateFailed => "preference_update_failed",
            Self::BleConnectionClosing => "ble_connection_closing",
            Self::BlePairingCapacityExceeded => "ble_pairing_capacity_exceeded",
            Self::ResyncRequired => "resync_required",
            Self::NetworkError => "network_error",
            Self::NativeError => "native_error",
            Self::NativePeerIdMissing => "native_peer_id_missing",
            Self::InvalidRequest => "invalid_request",
        }
    }

    pub fn from_error(error: &plain_libs::anyhow::Error) -> Self {
        error
            .downcast_ref::<Self>()
            .copied()
            .or_else(|| {
                error
                    .chain()
                    .find_map(|cause| cause.downcast_ref::<Self>())
                    .copied()
            })
            .unwrap_or(Self::InternalError)
    }
}

#[cfg(feature = "prefs")]
impl From<plain_libs::auth::client::LoginError> for ErrorCode {
    fn from(error: plain_libs::auth::client::LoginError) -> Self {
        use plain_libs::auth::client::LoginError;
        match error {
            LoginError::Randomness => Self::SecureLoginUnavailable,
            LoginError::Start | LoginError::SigningKey | LoginError::Response => {
                Self::InvalidAuthResponse
            }
            LoginError::Password => Self::InvalidPassword,
            LoginError::PeerIdentity => Self::PeerIdentityChanged,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for ErrorCode {}

impl std::str::FromStr for ErrorCode {
    type Err = serde::de::value::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::deserialize(serde::de::value::StrDeserializer::new(value))
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/infrastructure/error_code.rs"]
mod tests;

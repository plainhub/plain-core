#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum SignaturePurpose {
    #[serde(rename = "WEB_APPROVAL_RESPONSE")]
    WebApprovalResponse,
    #[serde(rename = "CHAT_PAIR_REQUEST")]
    ChatPairRequest,
    #[serde(rename = "PAIRING_ACCEPT")]
    PairingAccept,
    #[serde(rename = "PAIRING_REJECT")]
    PairingReject,
    #[serde(rename = "PAIR_CANCEL")]
    PairCancel,
    #[serde(rename = "PEER_REQUEST")]
    PeerRequest,
    #[serde(rename = "PEER_STATUS")]
    PeerStatus,
    #[serde(rename = "QR_PAIR_ACCEPT")]
    QrPairAccept,
    #[serde(rename = "CHANNEL_MEMBERSHIP")]
    ChannelMembership,
    #[serde(rename = "PEER_TLS_CERTIFICATE")]
    PeerTlsCertificate,
}

#[cfg(all(test, feature = "crypto"))]
#[path = "../../../tests/plain-rs/unit/infrastructure/signature.rs"]
mod tests;

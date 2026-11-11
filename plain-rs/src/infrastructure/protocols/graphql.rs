#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeerSubscriptionTarget {
    pub peer_id: String,
    pub public_key: String,
}

#[cfg(feature = "graphql")]
pub mod messages;

pub mod events;

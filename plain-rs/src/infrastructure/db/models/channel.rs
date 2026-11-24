use crate::features::chat::channels::messages::decode_members;
use crate::features::chat::enums::ChannelStatus;
use crate::infrastructure::db::now_iso;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DChannel {
    pub id: String,
    pub name: String,
    #[serde(
        default,
        serialize_with = "super::serialize_optional_text",
        deserialize_with = "super::deserialize_optional_text"
    )]
    pub owner_id: Option<String>,
    pub members: String,
    pub version: i64,
    pub status: ChannelStatus,
    pub created_at: String,
    pub updated_at: String,
}

impl DChannel {
    pub fn new(name: &str, owner: &str) -> Self {
        let now = now_iso();
        Self {
            id: crate::features::record_id::generate(),
            name: name.to_string(),
            owner_id: (!owner.is_empty()).then(|| owner.to_owned()),
            members: "[]".to_string(),
            version: 1,
            status: ChannelStatus::Joined,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn joined_member_ids(&self) -> Vec<String> {
        decode_members(&self.members)
            .into_iter()
            .filter(|m| m.is_joined())
            .map(|m| m.peer_id)
            .collect()
    }
}

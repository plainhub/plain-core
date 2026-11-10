use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Verification {
    Finish { request_signature: String, proof: String },
    Ack { request_signature: String, proof: String },
}

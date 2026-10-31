use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RECEIPTS_KEY: &str = "sms_send_requests";
const RECEIPT_TTL_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_RECEIPTS: usize = 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    key: String,
    payload_hash: String,
    created_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub number: String,
    pub body: String,
    pub subscription_id: Option<i32>,
    pub client_id: Option<String>,
    pub client_request_id: Option<String>,
}

fn permission_allowed(configured: &[String]) -> bool {
    configured.iter().any(|name| name == "SEND_SMS")
}

fn digest(bytes: &[u8]) -> String {
    plain_rs::utils::hex::bytes_to_hex(&Sha256::digest(bytes))
}

fn claim(
    receipts: &mut Vec<Receipt>,
    key: &str,
    payload_hash: &str,
    now_ms: u64,
) -> anyhow::Result<bool> {
    receipts.retain(|receipt| now_ms.saturating_sub(receipt.created_at_ms) < RECEIPT_TTL_MS);
    if let Some(existing) = receipts.iter().find(|receipt| receipt.key == key) {
        anyhow::ensure!(
            existing.payload_hash == payload_hash,
            "requestId was already used for a different SMS"
        );
        return Ok(false);
    }
    if receipts.len() >= MAX_RECEIPTS {
        let remove_count = receipts.len() - MAX_RECEIPTS + 1;
        receipts.drain(..remove_count);
    }
    receipts.push(Receipt {
        key: key.to_owned(),
        payload_hash: payload_hash.to_owned(),
        created_at_ms: now_ms,
    });
    Ok(true)
}

/// One send queue for the whole process, shared by the provider route and
/// the public `/graphql` root: the claim-then-send window must not be
/// interleaved with itself through a second entry point.
static SEND_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn execute(
    prefs: &Prefs,
    host: &Host,
    sends: &plain_rs::services::sms::send_runtime::Runtime,
    request: Request,
) -> anyhow::Result<Value> {
    anyhow::ensure!(!request.number.trim().is_empty(), "phone number is empty");
    let configured: Vec<String> =
        prefs.get_or(plain_rs::prefs::pref_keys::API_PERMISSIONS, Vec::new());
    anyhow::ensure!(permission_allowed(&configured), "no_permission");
    plain_rs::services::capabilities::require(host, "SMS").await?;
    let request_id = request
        .client_request_id
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let receipt_key = request_id.map(|request_id| {
        digest(
            format!(
                "{}\0{}",
                request.client_id.as_deref().unwrap_or_default(),
                request_id
            )
            .as_bytes(),
        )
    });
    if let Some(key) = &receipt_key {
        let _guard = SEND_LOCK.lock().await;
        let payload_hash = digest(
            serde_json::to_vec(&json!({
                "number": &request.number,
                "body": &request.body,
                "subscriptionId": request.subscription_id,
            }))?
            .as_slice(),
        );
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64;
        let mut receipts: Vec<Receipt> = sends.store()?.get_json(RECEIPTS_KEY)?.unwrap_or_default();
        if !claim(&mut receipts, key, &payload_hash, now_ms)? {
            return Ok(json!({"sent":true,"duplicate":true}));
        }
        // Persist the idempotency claim before asking the platform to dispatch.
        // This makes an ambiguous timeout/retry at-most-once across restarts.
        sends.store()?.set_json(RECEIPTS_KEY, &receipts)?;
    }
    let parts: Vec<String> = serde_json::from_value(
        host.call(
            "systemDivideSmsText",
            json!({"body":request.body,"subscriptionId":request.subscription_id}),
        )
        .await
        .map_err(anyhow::Error::msg)?,
    )?;
    anyhow::ensure!(
        !parts.is_empty() && parts.len() <= 1024,
        "Invalid SMS part count"
    );
    let request_id = uuid::Uuid::new_v4().to_string();
    sends.register(request_id.clone(), request.client_request_id, parts.len())?;
    let sent = host.call("systemSendSmsParts", json!({"requestId":request_id,"number":request.number,"parts":parts,"subscriptionId":request.subscription_id})).await;
    match sent {
        Ok(Value::Bool(true)) => {}
        Ok(_) => {
            sends.cancel(&request_id)?;
            anyhow::bail!("SMS dispatch was rejected");
        }
        Err(error) => {
            sends.cancel(&request_id)?;
            anyhow::bail!(error);
        }
    }
    Ok(json!({"sent":true}))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/send.rs"]
mod tests;

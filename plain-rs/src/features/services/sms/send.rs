use crate::features::prefs::Prefs;
use crate::infrastructure::host::bridge::Host;
use plain_libs::serde_json::{Value, json};
use serde::{Deserialize, Serialize};
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

pub struct Request {
    pub number: String,
    pub body: String,
    pub subscription_id: Option<i32>,
    pub attachment_paths: Vec<String>,
    pub request_id: String,
}

pub struct SendReceipt {
    pub request_id: String,
    pub requires_confirmation: bool,
}

fn permission_allowed(configured: &[String]) -> bool {
    configured.iter().any(|name| name == "SMS")
}

fn digest(bytes: &[u8]) -> String {
    plain_libs::utils::hex::bytes_to_hex(&Sha256::digest(bytes))
}

fn claim(
    receipts: &mut Vec<Receipt>,
    key: &str,
    payload_hash: &str,
    now_ms: u64,
) -> plain_libs::anyhow::Result<bool> {
    receipts.retain(|receipt| now_ms.saturating_sub(receipt.created_at_ms) < RECEIPT_TTL_MS);
    if let Some(existing) = receipts.iter().find(|receipt| receipt.key == key) {
        plain_libs::anyhow::ensure!(
            existing.payload_hash == payload_hash,
            "requestId was already used for a different message"
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

// Serialize durable idempotency claims before platform dispatch.
static SEND_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn execute(
    prefs: &Prefs,
    host: &Host,
    sends: &crate::features::services::sms::send_runtime::Runtime,
    mms: &std::sync::Arc<super::mms::Runtime>,
    request: Request,
) -> plain_libs::anyhow::Result<SendReceipt> {
    plain_libs::anyhow::ensure!(!request.number.trim().is_empty(), "phone number is empty");
    plain_libs::anyhow::ensure!(
        plain_libs::utils::uuid::Uuid::parse_str(&request.request_id).is_ok(),
        "requestId must be a UUID"
    );
    let configured: Vec<String> = prefs.get_or(
        crate::features::prefs::pref_keys::API_PERMISSIONS,
        Vec::new(),
    );
    plain_libs::anyhow::ensure!(permission_allowed(&configured), "no_permission");
    crate::features::services::capabilities::require(host, "TELEPHONY").await?;
    let requires_confirmation = !request.attachment_paths.is_empty();
    plain_libs::anyhow::ensure!(
        !requires_confirmation || request.subscription_id.is_none(),
        "subscriptionId cannot select a SIM for MMS; choose it in the phone's messaging app"
    );
    let receipt = SendReceipt {
        request_id: request.request_id,
        requires_confirmation,
    };
    let receipt_key = digest(receipt.request_id.as_bytes());
    {
        let _guard = SEND_LOCK.lock().await;
        let payload_hash = digest(
            plain_libs::serde_json::to_vec(&json!({
                "number": &request.number,
                "body": &request.body,
                "subscriptionId": request.subscription_id,
                "attachmentPaths": &request.attachment_paths,
            }))?
            .as_slice(),
        );
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64;
        let mut receipts: Vec<Receipt> = sends.store()?.get_json(RECEIPTS_KEY)?.unwrap_or_default();
        if !claim(&mut receipts, &receipt_key, &payload_hash, now_ms)? {
            return Ok(receipt);
        }
        // Persist before dispatch so ambiguous retries cannot send again after a restart.
        sends.store()?.set_json(RECEIPTS_KEY, &receipts)?;
    }
    if requires_confirmation {
        mms.send(
            receipt.request_id.clone(),
            request.number,
            request.body,
            request.attachment_paths,
        )
        .await?;
        return Ok(receipt);
    }
    let parts: Vec<String> = plain_libs::serde_json::from_value(
        host.call(
            "systemDivideSmsText",
            json!({"body":request.body,"subscriptionId":request.subscription_id}),
        )
        .await
        .map_err(plain_libs::anyhow::Error::msg)?,
    )?;
    plain_libs::anyhow::ensure!(
        !parts.is_empty() && parts.len() <= 1024,
        "Invalid SMS part count"
    );
    let request_id = &receipt.request_id;
    sends.register(request_id.to_owned(), parts.len())?;
    let sent = host.call("systemSendSmsParts", json!({"requestId":request_id,"number":request.number,"parts":parts,"subscriptionId":request.subscription_id})).await;
    match sent {
        Ok(Value::Bool(true)) => {}
        Ok(_) => {
            sends.cancel(request_id)?;
            plain_libs::anyhow::bail!("SMS dispatch was rejected");
        }
        Err(error) => {
            sends.cancel(request_id)?;
            plain_libs::anyhow::bail!(error);
        }
    }
    Ok(receipt)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/sms/send.rs"]
mod tests;

use plain_rs::services::casting::sender::types::DiscoveredDevice;
use plain_rs::services::casting::sender::util::xml_escape;
use std::time::Duration;

const SOAP_TIMEOUT: Duration = Duration::from_secs(3);
const SOAP_BODY_CAP: usize = 512 << 10;

/// Resolve a device's `controlURL` (which can be relative or absolute)
/// against its `Location` base. Mirrors Go `resolveServiceEndpoint`.
pub fn resolve_service_endpoint(location: &str, control_url: &str) -> Result<String, String> {
    // `loc` is only used to validate `location`; if it's not a parseable
    // URL we bail out. The actual base for the relative resolve is
    // `location.trim()` itself.
    if plain_rs::utils::http_url::parse_http_url(location.trim()).is_none() {
        return Err("parse location".to_string());
    }
    let control = control_url.trim();
    if control.is_empty() {
        return Err("empty controlURL".to_string());
    }
    // Absolute control URL? Use as-is. Relative? Resolve against `location`.
    if plain_rs::utils::http_url::parse_http_url(control).is_some() {
        return Ok(control.to_string());
    }
    let resolved = plain_rs::utils::http_url::join(location.trim(), control)
        .ok_or_else(|| "resolve reference".to_string())?;
    // We have to re-parse the result to check it has a host — but we
    // just built it from a base that did, so this is paranoia.
    if plain_rs::utils::http_url::parse_http_url(&resolved)
        .map(|u| u.host.is_empty())
        .unwrap_or(true)
    {
        return Err("invalid resolved endpoint".to_string());
    }
    Ok(resolved)
}

pub fn envelope(st: &str, action: &str, params_xml: &str) -> String {
    let action_esc = xml_escape(action);
    let st_esc = xml_escape(st);
    let body_inner = format!("<u:{action_esc} xmlns:u=\"{st_esc}\">{params_xml}</u:{action_esc}>");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>{body_inner}</s:Body>\
</s:Envelope>"
    )
}

pub async fn soap_response(
    dev: &DiscoveredDevice,
    action: &str,
    params: &str,
    sender_name: &str,
) -> Result<String, String> {
    let st = &dev.av_transport.service_type;
    if st.is_empty() {
        return Err("missing AVTransport serviceType".into());
    }
    let endpoint = resolve_service_endpoint(&dev.location, &dev.av_transport.control_url)?;
    let client = reqwest::Client::builder()
        .timeout(SOAP_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .post(endpoint)
        .header("Content-Type", "text/xml")
        .header("SOAPAction", format!("\"{st}#{action}\""))
        .header("c-name", sender_name)
        .body(envelope(st, action, params))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("soap {action} http {}", response.status()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > SOAP_BODY_CAP {
            return Err("SOAP response too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
pub struct Subscription {
    pub sid: String,
    pub renew_after: std::time::Duration,
}

pub async fn subscription(
    dev: &DiscoveredDevice,
    callback: Option<&str>,
    sid: &str,
    unsubscribe: bool,
) -> Result<Subscription, String> {
    let endpoint = resolve_service_endpoint(&dev.location, &dev.av_transport.event_sub_url)?;
    let client = reqwest::Client::builder()
        .timeout(SOAP_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let method = reqwest::Method::from_bytes(if unsubscribe {
        b"UNSUBSCRIBE"
    } else {
        b"SUBSCRIBE"
    })
    .map_err(|e| e.to_string())?;
    let mut request = client.request(method, endpoint);
    if !sid.is_empty() {
        request = request.header("SID", sid);
    }
    if !unsubscribe {
        request = request.header("TIMEOUT", "Second-3600");
        if let Some(callback) = callback {
            request = request
                .header("NT", "upnp:event")
                .header("CALLBACK", format!("<{callback}>"));
        }
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("subscription http {}", response.status()));
    }
    let lifetime = response
        .headers()
        .get("TIMEOUT")
        .and_then(|v| v.to_str().ok())
        .and_then(|value| value.split_once('-'))
        .filter(|(kind, _)| kind.eq_ignore_ascii_case("second"))
        .and_then(|(_, seconds)| seconds.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(3600);
    Ok(Subscription {
        sid: response
            .headers()
            .get("SID")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned(),
        renew_after: std::time::Duration::from_millis(lifetime.saturating_mul(500).max(1)),
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/casting/sender/soap.rs"]
mod tests;

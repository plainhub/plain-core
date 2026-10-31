use plain_rs::host::bridge::Host;
#[cfg(target_os = "android")]
use plain_rs::platform::network::lan::apply_interface_facts;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use plain_rs::platform::network::lan::classify_network;
use plain_rs::platform::network::lan::{NetworkType, select_lan_ipv4_addresses};

pub async fn ipv4(host: &Host) -> Result<String, String> {
    let value = lan_ipv4(host).await?;
    if value.is_empty() {
        return Err("No LAN address available for casting".into());
    }
    let address: std::net::Ipv4Addr = value.parse().map_err(|_| "Invalid IPv4 reading")?;
    if address.is_unspecified() || address.is_loopback() {
        return Err("No LAN address available for casting".into());
    }
    Ok(address.to_string())
}

pub async fn lan_ipv4_addresses(host: &Host) -> Result<Vec<String>, String> {
    let interfaces = tokio::task::spawn_blocking(|| {
        let interfaces =
            plain_rs::platform::network::interfaces().map_err(|error| error.to_string())?;
        Ok::<_, String>(interfaces)
    })
    .await
    .map_err(|error| error.to_string())??;
    #[cfg(target_os = "android")]
    let interfaces = {
        let facts = serde_json::from_value(
            host.call("systemNetworkInterfaceFacts", serde_json::json!({}))
                .await?,
        )
        .map_err(|error| error.to_string())?;
        apply_interface_facts(interfaces, facts)
    };
    #[cfg(not(target_os = "android"))]
    let _ = host;
    Ok(select_lan_ipv4_addresses(&interfaces))
}

pub async fn lan_ipv4(host: &Host) -> Result<String, String> {
    Ok(lan_ipv4_addresses(host)
        .await?
        .into_iter()
        .next()
        .unwrap_or_default())
}

pub async fn network_type(host: &Host) -> Result<NetworkType, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        serde_json::from_value(
            host.call("systemNetworkType", serde_json::json!({}))
                .await?,
        )
        .map_err(|error| error.to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = host;
        tokio::task::spawn_blocking(|| {
            classify_network(
                plain_rs::platform::network::default_interface()
                    .map(|interface| interface.map(|interface| interface.if_type)),
            )
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

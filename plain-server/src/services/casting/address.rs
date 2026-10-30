use crate::assembly::server::ServerState;

pub(crate) async fn device_name(state: &ServerState) -> Result<String, String> {
    let settings =
        plain_rs::prefs::settings::snapshot(&state.prefs).map_err(|error| error.to_string())?;
    if !settings.device_name.is_empty() {
        return Ok(settings.device_name);
    }
    Ok(state.host.startup().name.clone())
}

fn http_base(ip: &str, port: u16) -> Result<String, String> {
    let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| "Invalid cast address")?;
    if port == 0 {
        return Err("Active HTTP port required for casting".into());
    }
    Ok(if port == 80 {
        format!("http://{ip}")
    } else {
        format!("http://{ip}:{port}")
    })
}

pub(crate) async fn http_address(state: &ServerState) -> Result<(String, String), String> {
    let (ip, name) = tokio::try_join!(
        crate::platform::network::ipv4(&state.host),
        device_name(state)
    )?;
    let port = match state.public_ports().await {
        Some((http, _)) => http,
        None => u16::try_from(
            plain_rs::prefs::settings::snapshot(&state.prefs)
                .map_err(|error| error.to_string())?
                .http_port,
        )
        .map_err(|error| error.to_string())?,
    };
    Ok((http_base(&ip, port)?, name))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/casting/address.rs"]
mod tests;

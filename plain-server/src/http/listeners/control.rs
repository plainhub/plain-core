use crate::server::ServerState;
use plain_rs::services::http_status::HttpServerState;
pub(crate) fn restart(
    state: &ServerState,
    http: Option<u16>,
    https: Option<u16>,
) -> Result<HttpServerState, String> {
    state.native_client_id()?;
    #[cfg(feature = "http_transport")]
    {
        let receipt = state.http_server.begin()?;
        let revision = receipt.revision;
        let state = state.clone();
        tokio::spawn(async move {
            let _shutdown = state.shutdown.lock().await;
            let result = async {
                state.native_client_id()?;
                let preferences =
                    plain_rs::services::settings::preferences::Preferences::from_server(&state);
                preferences
                    .patch(plain_rs::prefs::settings::SettingsPatch {
                        http_port: http.map(i32::from),
                        https_port: https.map(i32::from),
                        ..Default::default()
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                state.restart_public_locked().await
            }
            .await;
            state.http_server.finish(revision, result);
            state.publish_http_status();
        });
        Ok(receipt)
    }
    #[cfg(not(feature = "http_transport"))]
    {
        let _ = (http, https);
        Err("HTTP transport is unavailable".into())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/http/listeners/control.rs"]
mod tests;

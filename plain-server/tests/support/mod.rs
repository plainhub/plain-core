pub(crate) async fn graphql_http(port: u16, token: &str, query: &str) -> serde_json::Value {
    graphql_http_variables(port, token, query, serde_json::json!({})).await
}

pub(crate) async fn graphql_http_variables(
    port: u16,
    token: &str,
    query: &str,
    variables: serde_json::Value,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/graphql"))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(serde_json::json!({"query":query,"variables":variables}).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap()
}

pub(crate) fn acknowledge_host_callback(
    host: &plain_rs::host::bridge::Host,
    generation: u64,
    request: &serde_json::Value,
) -> bool {
    if !matches!(
        request["method"].as_str(),
        Some(
            "systemWebLoginRequested"
                | "systemWebLoginClosed"
                | "systemWebLoginCompleted"
                | "systemWebLoginSnapshot"
                | "systemWebActivity"
                | "peerNotificationsChanged"
                | "peerClipboardChanged"
                | "onlineClientsChanged"
                | "httpServerStatusChanged"
                | "clipboardHistoryRefresh"
                | "dlnaRendererChanged"
                | "dlnaRendererRefresh"
                | "sharedFolderDownloadsRefresh"
                | "peerTransportsChanged"
                | "appUpdateDownloadChanged"
                | "dlnaSenderChanged"
                | "feedSyncChanged"
                | "audioPlaybackRefresh"
                | "audioLibraryRefresh"
                | "networkInterfacesChanged"
                | "pairingSucceeded"
        )
    ) {
        return false;
    }
    host.reply(
        generation,
        serde_json::json!({"id":request["id"],"result":null}),
    )
    .unwrap();
    true
}

pub(crate) struct HostRequests {
    host: std::sync::Arc<plain_rs::host::bridge::Host>,
    generation: u64,
    incoming: tokio::sync::mpsc::Receiver<serde_json::Value>,
}

impl HostRequests {
    pub(crate) fn connect(host: &std::sync::Arc<plain_rs::host::bridge::Host>) -> (u64, Self) {
        let (generation, incoming) = host.connect();
        (
            generation,
            Self {
                host: host.clone(),
                generation,
                incoming,
            },
        )
    }
    pub(crate) async fn recv(&mut self) -> Option<serde_json::Value> {
        loop {
            let request = self.incoming.recv().await?;
            if !acknowledge_host_callback(&self.host, self.generation, &request) {
                return Some(request);
            }
        }
    }
    pub(crate) fn try_recv(
        &mut self,
    ) -> Result<serde_json::Value, tokio::sync::mpsc::error::TryRecvError> {
        loop {
            let request = self.incoming.try_recv()?;
            if !acknowledge_host_callback(&self.host, self.generation, &request) {
                return Ok(request);
            }
        }
    }
}

pub(crate) mod schema;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub host: String,
    pub port: u16,
    pub shared_id: String,
    pub token: String,
    #[serde(default)]
    pub page_url: String,
}
impl Link {
    pub fn new(host: &str, port: u16, id: &str, token: &str) -> Result<Self> {
        let mut link = Self {
            host: host.trim().into(),
            port,
            shared_id: id.into(),
            token: token.into(),
            page_url: String::new(),
        };
        let mut url = link.url("/")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid share URL"))?
            .clear()
            .push("s")
            .push(id);
        url.set_fragment(Some(token));
        link.page_url = url.to_string();
        Ok(link)
    }
    pub fn url(&self, path: &str) -> Result<reqwest::Url> {
        ensure!(
            self.port > 0 && !self.host.is_empty() && !self.shared_id.is_empty(),
            "Invalid share endpoint"
        );
        ensure!(
            !self
                .host
                .chars()
                .any(|c| c.is_whitespace() || "/@?#,\\".contains(c)),
            "Invalid share host"
        );
        let url = reqwest::Url::parse(&crate::utils::build_url::build_url(
            "https", &self.host, self.port, path,
        ))?;
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.scheme() == "https"
                && url.host_str().is_some(),
            "Invalid share URL"
        );
        self.key()?;
        Ok(url)
    }
    pub fn key(&self) -> Result<Vec<u8>> {
        let key = crate::utils::base64::base64_decode_checked(&self.token)?;
        ensure!(key.len() == 32, "Invalid share token");
        Ok(key)
    }
    pub fn request(&self, relative_path: Option<&str>) -> Result<Vec<u8>> {
        let query = "query($relativePath: String) { sharedInfo(relativePath: $relativePath) { name readOnly requiresPassword expiresAt urlToken entries { name relativePath isDir size mimeType hasThumb } } }";
        let body = json!({"query":query,"variables":{"relativePath":relative_path}}).to_string();
        let envelope = format!(
            "{}|{}|{body}",
            crate::peers::pairing::now_ms(),
            uuid::Uuid::new_v4()
        );
        crate::xchacha_encrypt_raw(&self.key()?, envelope.as_bytes())
            .ok_or_else(|| anyhow::anyhow!("Cannot encrypt shared request"))
    }
    pub fn response(&self, request: &[u8], bytes: &[u8]) -> Result<Info> {
        let plain = crate::transport::http_response::Context::new("/guest_graphql", request)
            .decrypt(&self.key()?, bytes)
            .ok_or_else(|| anyhow::anyhow!("Invalid shared response authentication"))?;
        let response: Value = serde_json::from_slice(&plain)?;
        if let Some(error) = response
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
        {
            anyhow::bail!(
                "{}",
                error["message"].as_str().unwrap_or("Share request failed")
            );
        }
        let info: Info = serde_json::from_value(response["data"]["sharedInfo"].clone())?;
        ensure!(
            crate::utils::base64::base64_decode_checked(&info.url_token)?.len() == 32,
            "Invalid shared file token"
        );
        Ok(info)
    }
    pub fn file_url(&self, url_token: &str, relative_path: &str, zip: bool) -> Result<String> {
        let key = crate::utils::base64::base64_decode_checked(url_token)?;
        ensure!(key.len() == 32, "Invalid shared file token");
        let id = crate::files::resource_link::seal(
            &crate::files::resource_link::Scope::Share(self.shared_id.clone()),
            &key,
            &crate::files::resource_link::Resource {
                r#type: crate::files::resource_link::ResourceType::SharedFile,
                id: relative_path.into(),
            },
        )?;
        let mut url = self.url(if zip { "/zip/dir" } else { "/fs" })?;
        url.query_pairs_mut().append_pair("id", &id);
        Ok(url.to_string())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub size: i64,
    pub mime_type: String,
    pub has_thumb: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub name: String,
    pub read_only: bool,
    pub requires_password: bool,
    pub expires_at: Option<i64>,
    pub url_token: String,
    pub entries: Vec<File>,
}
#[cfg(test)]
#[path = "../../../tests/unit/files/shares/client.rs"]
mod tests;

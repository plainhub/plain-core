use plain_libs::anyhow::{Result, ensure};
use plain_libs::protocols::http_client;
use plain_libs::serde_json::{Value, json};
use serde::{Deserialize, Serialize};
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
    pub fn parse(raw: &str) -> Result<(Self, String)> {
        let url = http_client::Url::parse(raw)?;
        let segments: Vec<_> = url
            .path_segments()
            .into_iter()
            .flatten()
            .map(|part| plain_libs::utils::query::percent_decode(&part.replace('+', "%2B")))
            .collect();
        ensure!(
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && segments.len() >= 2
                && segments[0] == "s"
                && !segments[1].is_empty(),
            "Invalid share URL"
        );
        let link = Self::new(
            url.host_str()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing share host"))?,
            url.port_or_known_default().unwrap_or(443),
            &segments[1],
            url.fragment()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing share token"))?,
        )?;
        let path = segments[2..].join("/");
        Ok((link, path))
    }
    pub fn directory_url(&self, path: &str) -> Result<String> {
        let mut url = http_client::Url::parse(&self.page_url)?;
        url.path_segments_mut()
            .map_err(|_| plain_libs::anyhow::anyhow!("Invalid share path"))?
            .extend(
                path.trim_matches('/')
                    .split('/')
                    .filter(|part| !part.is_empty()),
            );
        Ok(url.to_string())
    }
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
            .map_err(|_| plain_libs::anyhow::anyhow!("Invalid share URL"))?
            .clear()
            .push("s")
            .push(id);
        url.set_fragment(Some(token));
        link.page_url = url.to_string();
        Ok(link)
    }
    pub fn url(&self, path: &str) -> Result<http_client::Url> {
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
        let url = http_client::Url::parse(&plain_libs::utils::build_url::build_url(
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
        let key = plain_libs::utils::base64::base64_decode_checked(&self.token)?;
        ensure!(key.len() == 32, "Invalid share token");
        Ok(key)
    }
    pub fn request(&self, relative_path: Option<&str>) -> Result<Vec<u8>> {
        let query = "query($relativePath: String) { sharedInfo(relativePath: $relativePath) { name readOnly requiresPassword expiresAt urlToken entries { name relativePath isDir size mimeType hasThumb createdAt updatedAt } } }";
        let body = json!({"query":query,"variables":{"relativePath":relative_path}}).to_string();
        crate::infrastructure::transport::http_request::encrypt(
            &self.key()?,
            "/guest_graphql",
            &body,
        )
        .map_err(|_| plain_libs::anyhow::anyhow!("Cannot encrypt shared request"))
    }
    pub fn response(&self, request: &[u8], bytes: &[u8]) -> Result<Info> {
        let plain = crate::infrastructure::transport::http_response::Context::new(request)
            .decrypt(&self.key()?, bytes)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid shared response authentication"))?;
        let response: Value = plain_libs::serde_json::from_slice(&plain)?;
        if let Some(error) = response
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
        {
            plain_libs::anyhow::bail!(
                "{}",
                error["message"].as_str().unwrap_or("Share request failed")
            );
        }
        let info: Info =
            plain_libs::serde_json::from_value(response["data"]["sharedInfo"].clone())?;
        ensure!(
            plain_libs::utils::base64::base64_decode_checked(&info.url_token)?.len() == 32,
            "Invalid shared file token"
        );
        Ok(info)
    }
    pub fn file_url(&self, url_token: &str, relative_path: &str, zip: bool) -> Result<String> {
        let key = plain_libs::utils::base64::base64_decode_checked(url_token)?;
        ensure!(key.len() == 32, "Invalid shared file token");
        let id = crate::features::files::resource_link::seal(
            &crate::features::files::resource_link::Scope::Share(self.shared_id.clone()),
            &key,
            &crate::features::files::resource_link::Resource {
                r#type: crate::features::files::resource_link::ResourceType::SharedFile,
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
    pub created_at: Option<crate::infrastructure::types::Instant>,
    pub updated_at: crate::infrastructure::types::Instant,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub name: String,
    pub read_only: bool,
    pub requires_password: bool,
    pub expires_at: Option<crate::infrastructure::types::Instant>,
    pub url_token: String,
    pub entries: Vec<File>,
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/files/shares/client.rs"]
mod tests;

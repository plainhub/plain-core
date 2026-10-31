use crate::features::prefs::security::UpdateInfo;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub url: String,
    pub assets: Vec<AssetRule>,
    #[serde(default)]
    pub page_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetRule {
    #[serde(default)]
    pub required_abis: Vec<String>,
    #[serde(default)]
    pub name_contains: String,
    #[serde(default)]
    pub name_suffix: String,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub browser_download_url: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version([i32; 3]);
impl Version {
    fn parse(text: &str) -> Self {
        let token = numeric_version(text).unwrap_or("");
        let mut parts = token.split('.');
        Self(std::array::from_fn(|_| {
            parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        }))
    }
    fn text(&self) -> String {
        format!("{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}

fn numeric_version(text: &str) -> Option<&str> {
    let parts = || {
        text.split(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|part| part.trim_matches('.'))
            .filter(|part| !part.is_empty())
    };
    parts()
        .find(|part| part.contains('.'))
        .or_else(|| parts().next())
}
pub fn normalized_version(text: &str) -> String {
    if numeric_version(text).is_some() {
        Version::parse(text).text()
    } else {
        text.trim().to_owned()
    }
}
pub fn release_info(
    release: &Release,
    source: &Source,
) -> anyhow::Result<crate::features::prefs::security::ReleaseInfo> {
    anyhow::ensure!(
        numeric_version(&release.tag_name).is_some(),
        "Invalid release version"
    );
    Ok(crate::features::prefs::security::ReleaseInfo {
        version: Version::parse(&release.tag_name).text(),
        name: release.name.clone(),
        url: if release.html_url.trim().is_empty() {
            source.page_url.clone()
        } else {
            release.html_url.trim().to_owned()
        },
        published_at: if release.published_at.is_empty() {
            release.created_at.clone()
        } else {
            release.published_at.clone()
        },
    })
}
pub fn available_version(latest: &str, current: &str, skip: &str) -> bool {
    let latest = Version::parse(latest);
    latest > Version::parse(current) && latest > Version::parse(skip)
}
pub fn available(info: &UpdateInfo, current: &str, skip: &str) -> bool {
    available_version(&info.new_version, current, skip)
}

pub fn due(info: &UpdateInfo, auto_check: bool, now_ms: u64) -> bool {
    auto_check && now_ms.saturating_sub(info.check_update_time) > 86_400_000
}

pub fn apply_release(
    info: &mut UpdateInfo,
    current: &str,
    skip: &str,
    abis: &[String],
    source: &Source,
    release: &Release,
) -> bool {
    let latest = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    );
    if latest <= Version::parse(current) || latest <= Version::parse(skip) {
        return false;
    }
    let rule = source.assets.iter().find(|rule| {
        rule.required_abis.is_empty() || rule.required_abis.iter().any(|abi| abis.contains(abi))
    });
    let asset = rule.and_then(|rule| {
        release.assets.iter().find(|asset| {
            asset.name.contains(&rule.name_contains) && asset.name.ends_with(&rule.name_suffix)
        })
    });
    info.new_version = latest.text();
    info.log = release.body.clone();
    info.publish_date = if release.published_at.is_empty() {
        &release.created_at
    } else {
        &release.published_at
    }
    .clone();
    info.size = asset.map(|asset| asset.size).unwrap_or(0);
    info.download_url = asset
        .map(|asset| asset.browser_download_url.clone())
        .unwrap_or_default();
    true
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/updates.rs"]
mod tests;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateInfo {
    pub new_version: String,
    pub latest_release: Option<ReleaseInfo>,
    pub check_update_time: u64,
    pub publish_date: String,
    pub log: String,
    pub download_url: String,
    pub size: u64,
    pub downloaded_apk_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePatch {
    pub new_version: Option<String>,
    pub latest_release: Option<ReleaseInfo>,
    pub check_update_time: Option<u64>,
    pub publish_date: Option<String>,
    pub log: Option<String>,
    pub download_url: Option<String>,
    pub size: Option<u64>,
    pub downloaded_apk_path: Option<String>,
}

impl UpdatePatch {
    pub fn apply(self, info: &mut UpdateInfo) {
        macro_rules! patch { ($($field:ident),*) => { $(if let Some(value) = self.$field { info.$field = value; })* }; }
        if let Some(release) = self.latest_release {
            info.latest_release = Some(release);
        }
        patch!(
            new_version,
            check_update_time,
            publish_date,
            log,
            download_url,
            size,
            downloaded_apk_path
        );
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub name: String,
    pub url: String,
    pub published_at: String,
}

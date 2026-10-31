use plain_rs::chat::enums::DeviceType;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartupConfig {
    pub debug: bool,
    pub cache_dir: Option<String>,
    pub name: String,
    pub device_type: DeviceType,
    pub build_channel: String,
    pub downloads_dir: String,
    pub manufacturer: String,
    pub model: String,
    pub os_name: String,
    pub os_version: String,
    pub app_version: String,
    pub app_build_number: String,
    pub cpu_arch: String,
    pub soc_model: Option<String>,
    pub wifi_aware_supported: bool,
    pub recent_files_catalog: bool,
    pub system_media_catalog: bool,
    pub supported_abis: Vec<String>,
    pub update_source: Option<plain_rs::updates::Source>,
    pub android_sdk_version: Option<i32>,
    pub android_version_code_name: Option<String>,
    pub android_security_patch: Option<String>,
    pub android_bootloader: Option<String>,
    pub android_fingerprint: Option<String>,
    pub android_hardware: Option<String>,
    pub android_radio_version: Option<String>,
    pub android_board: Option<String>,
    pub android_build_brand: Option<String>,
    pub android_build_number: Option<String>,
    pub android_device: Option<String>,
    pub android_java_vm_version: Option<String>,
    pub android_gl_es_version: Option<String>,
    pub android_build_time: Option<i64>,
}

impl StartupConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Startup device name required".into());
        }
        if self
            .cache_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("Absolute startup cache directory required".into());
        }
        if self.supported_abis.iter().any(String::is_empty) {
            return Err("Invalid startup supported ABIs".into());
        }
        if !matches!(self.build_channel.as_str(), "GITHUB" | "GOOGLE" | "FDROID") {
            return Err("Invalid startup build channel".into());
        }
        if self.device_type == DeviceType::Unknown {
            return Err("Invalid startup device type".into());
        }
        if self.android_sdk_version.is_some_and(|version| version <= 0) {
            return Err("Invalid startup Android SDK version".into());
        }
        Ok(())
    }
}

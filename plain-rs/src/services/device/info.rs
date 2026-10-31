use plain_rs::services::device::types::{AndroidExtras, DeviceInfo, DevicePlatform, DisplayInfo};

use plain_rs::types::{Instant, Long};

use plain_rs::host::bridge::Host;
use serde_json::{Map, Value};

pub const FIELDS: &[&str] = &[
    "name",
    "platform",
    "manufacturer",
    "model",
    "osName",
    "osVersion",
    "kernelVersion",
    "appVersion",
    "appBuildNumber",
    "language",
    "cpuArch",
    "cpuModel",
    "totalMemory",
    "totalStorage",
];
pub const DISPLAY_FIELDS: &[&str] = &["width", "height", "density"];
pub const ANDROID_FIELDS: &[&str] = &[
    "sdkVersion",
    "versionCodeName",
    "securityPatch",
    "bootloader",
    "fingerprint",
    "hardware",
    "radioVersion",
    "board",
    "buildBrand",
    "buildNumber",
    "device",
    "javaVmVersion",
    "glEsVersion",
    "buildTime",
];

pub fn sdk_properties(fields: &[String]) -> Vec<String> {
    if !cfg!(any(target_os = "android", target_os = "ios")) {
        return Vec::new();
    }
    fields
        .iter()
        .filter(|field| field.as_str() == "language" || field.starts_with("display."))
        .cloned()
        .collect()
}

pub fn startup(host: &Host) -> Result<DeviceInfo, String> {
    let fields: Vec<String> = FIELDS
        .iter()
        .filter(|field| !matches!(**field, "totalStorage" | "language"))
        .map(|field| (*field).to_owned())
        .collect();
    let mut info = direct(&fields, std::path::Path::new(""))?;
    let config = host.startup();
    info.name = config.name.clone();
    info.manufacturer = config.manufacturer.clone();
    info.model = config.model.clone();
    info.os_name = config.os_name.clone();
    info.os_version = config.os_version.clone();
    info.app_version = config.app_version.clone();
    info.app_build_number = config.app_build_number.clone();
    info.cpu_arch = config.cpu_arch.clone();
    if let Some(model) = config
        .soc_model
        .as_ref()
        .filter(|model| !model.trim().is_empty() && model.as_str() != "unknown")
    {
        info.cpu_model = Some(model.clone());
    }
    if let Some(sdk_version) = config.android_sdk_version {
        info.android = Some(AndroidExtras {
            sdk_version,
            version_code_name: config
                .android_version_code_name
                .clone()
                .ok_or("Missing Android version code name")?,
            security_patch: config
                .android_security_patch
                .clone()
                .ok_or("Missing Android security patch")?,
            bootloader: config
                .android_bootloader
                .clone()
                .ok_or("Missing Android bootloader")?,
            fingerprint: config
                .android_fingerprint
                .clone()
                .ok_or("Missing Android fingerprint")?,
            hardware: config
                .android_hardware
                .clone()
                .ok_or("Missing Android hardware")?,
            radio_version: config
                .android_radio_version
                .clone()
                .ok_or("Missing Android radio version")?,
            board: config
                .android_board
                .clone()
                .ok_or("Missing Android board")?,
            build_brand: config
                .android_build_brand
                .clone()
                .ok_or("Missing Android build brand")?,
            build_number: config
                .android_build_number
                .clone()
                .ok_or("Missing Android build number")?,
            device: config
                .android_device
                .clone()
                .ok_or("Missing Android device")?,
            java_vm_version: config
                .android_java_vm_version
                .clone()
                .ok_or("Missing Android Java VM version")?,
            gl_es_version: config
                .android_gl_es_version
                .clone()
                .ok_or("Missing Android GL ES version")?,
            build_time: Instant(
                chrono::DateTime::from_timestamp_millis(
                    config
                        .android_build_time
                        .ok_or("Missing Android build time")?,
                )
                .ok_or("Invalid Android build timestamp")?,
            ),
        });
    }
    Ok(info)
}

pub async fn collect(
    host: &Host,
    directory: std::path::PathBuf,
    fields: Vec<String>,
    mut fixed: DeviceInfo,
) -> Result<DeviceInfo, String> {
    let names = sdk_properties(&fields);
    let display = cfg!(any(target_os = "android", target_os = "ios"))
        && fields.iter().any(|name| name == "display");
    let sdk = read_properties(host, names);
    let direct = async move {
        if fields.iter().any(|field| field == "totalStorage") {
            tokio::task::spawn_blocking(move || {
                fixed.total_storage =
                    Long(plain_rs::services::device::storage_bytes(&directory, true)?);
                Ok::<_, String>(fixed)
            })
            .await
            .map_err(|error| error.to_string())?
        } else {
            Ok(fixed)
        }
    };
    let (sdk, direct) = tokio::join!(sdk, direct);
    apply_sdk(direct?, sdk?, display)
}

async fn read_properties(host: &Host, names: Vec<String>) -> Result<Map<String, Value>, String> {
    if names.is_empty() {
        return Ok(Map::new());
    }
    let values = host
        .call("systemProperties", serde_json::json!({"names":names}))
        .await?;
    let values = values.as_object().ok_or("invalid system properties")?;
    for name in &names {
        let value = values
            .get(name)
            .ok_or_else(|| format!("missing system property: {name}"))?;
        let valid = match name.as_str() {
            "display.width" | "display.height" => value.as_i64().is_some(),
            "display.density" => value
                .as_f64()
                .is_some_and(|value| value.is_finite() && value > 0.0),
            _ => value.is_string(),
        };
        if !valid {
            return Err(format!("invalid system property: {name}"));
        }
    }
    Ok(values.clone())
}

fn apply_sdk(
    mut info: DeviceInfo,
    sdk: Map<String, Value>,
    display: bool,
) -> Result<DeviceInfo, String> {
    if let Some(language) = sdk.get("language").and_then(Value::as_str) {
        info.language = language.to_owned();
    }
    if display || sdk.keys().any(|key| key.starts_with("display.")) {
        info.display = Some(DisplayInfo {
            width: sdk
                .get("display.width")
                .and_then(Value::as_i64)
                .unwrap_or_default() as i32,
            height: sdk
                .get("display.height")
                .and_then(Value::as_i64)
                .unwrap_or_default() as i32,
            density: sdk
                .get("display.density")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
        });
    }
    Ok(info)
}

fn direct(fields: &[String], directory: &std::path::Path) -> Result<DeviceInfo, String> {
    let selected = |name: &str| fields.iter().any(|field| field == name);
    let mut info = DeviceInfo {
        name: String::new(),
        platform: platform(),
        manufacturer: String::new(),
        model: String::new(),
        os_name: String::new(),
        os_version: String::new(),
        kernel_version: String::new(),
        app_version: String::new(),
        app_build_number: String::new(),
        language: String::new(),
        cpu_arch: String::new(),
        cpu_model: None,
        total_memory: Long(0),
        total_storage: Long(0),
        display: None,
        android: None,
    };
    if selected("kernelVersion") {
        #[cfg(target_os = "ios")]
        {
            info.kernel_version = plain_rs::services::device::apple::kernel_version()?;
        }
        #[cfg(not(target_os = "ios"))]
        {
            info.kernel_version = sysinfo::System::kernel_version().unwrap_or_default();
        }
    }
    if selected("totalMemory") {
        #[cfg(target_os = "ios")]
        {
            info.total_memory = Long(plain_rs::services::device::apple::total_memory()?);
        }
        #[cfg(not(target_os = "ios"))]
        {
            let mut system = sysinfo::System::new();
            system.refresh_memory();
            info.total_memory = Long(system.total_memory().min(i64::MAX as u64) as i64);
        }
    }
    if selected("totalStorage") {
        info.total_storage = Long(plain_rs::services::device::storage_bytes(directory, true)?);
    }
    #[cfg(not(target_os = "ios"))]
    if selected("cpuModel") {
        #[cfg(target_os = "android")]
        {
            info.cpu_model = std::fs::read_to_string("/proc/cpuinfo")
                .ok()
                .and_then(|text| cpu_model(&text));
        }
        #[cfg(not(target_os = "android"))]
        {
            let mut system = sysinfo::System::new();
            system.refresh_cpu_all();
            info.cpu_model = system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().to_owned())
                .filter(|value| !value.is_empty());
        }
    }
    Ok(info)
}

fn platform() -> DevicePlatform {
    match std::env::consts::OS {
        "android" => DevicePlatform::Android,
        "ios" => DevicePlatform::Ios,
        "macos" => DevicePlatform::Macos,
        "windows" => DevicePlatform::Windows,
        _ => DevicePlatform::Linux,
    }
}

fn cpu_model(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim() == "model name")
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/device/info.rs"]
mod tests;

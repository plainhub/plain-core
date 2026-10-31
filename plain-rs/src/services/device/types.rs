use crate::types::{Instant, Long};
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
pub enum DeviceType {
    #[default]
    Computer,
    Phone,
    Tablet,
    Tv,
    Nas,
    Other,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
pub enum DevicePlatform {
    #[default]
    Android,
    Ios,
    Macos,
    Windows,
    Linux,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
pub enum AppChannelType {
    #[default]
    Github,
    Google,
    Fdroid,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct DisplayInfo {
    pub width: i32,
    pub height: i32,
    pub density: f64,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct AndroidExtras {
    #[cfg_attr(feature = "graphql", graphql(name = "sdkVersion"))]
    pub sdk_version: i32,
    #[cfg_attr(feature = "graphql", graphql(name = "versionCodeName"))]
    pub version_code_name: String,
    #[cfg_attr(feature = "graphql", graphql(name = "securityPatch"))]
    pub security_patch: String,
    pub bootloader: String,
    pub fingerprint: String,
    pub hardware: String,
    #[cfg_attr(feature = "graphql", graphql(name = "radioVersion"))]
    pub radio_version: String,
    pub board: String,
    #[cfg_attr(feature = "graphql", graphql(name = "buildBrand"))]
    pub build_brand: String,
    #[cfg_attr(feature = "graphql", graphql(name = "buildNumber"))]
    pub build_number: String,
    pub device: String,
    #[cfg_attr(feature = "graphql", graphql(name = "javaVmVersion"))]
    pub java_vm_version: String,
    #[cfg_attr(feature = "graphql", graphql(name = "glEsVersion"))]
    pub gl_es_version: String,
    #[cfg_attr(feature = "graphql", graphql(name = "buildTime"))]
    pub build_time: Instant,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct DeviceInfo {
    pub name: String,
    pub platform: DevicePlatform,
    pub manufacturer: String,
    pub model: String,
    #[cfg_attr(feature = "graphql", graphql(name = "osName"))]
    pub os_name: String,
    #[cfg_attr(feature = "graphql", graphql(name = "osVersion"))]
    pub os_version: String,
    #[cfg_attr(feature = "graphql", graphql(name = "kernelVersion"))]
    pub kernel_version: String,
    #[cfg_attr(feature = "graphql", graphql(name = "appVersion"))]
    pub app_version: String,
    #[cfg_attr(feature = "graphql", graphql(name = "appBuildNumber"))]
    pub app_build_number: String,
    pub language: String,
    #[cfg_attr(feature = "graphql", graphql(name = "cpuArch"))]
    pub cpu_arch: String,
    #[cfg_attr(feature = "graphql", graphql(name = "cpuModel"))]
    pub cpu_model: Option<String>,
    #[cfg_attr(feature = "graphql", graphql(name = "totalMemory"))]
    pub total_memory: Long,
    #[cfg_attr(feature = "graphql", graphql(name = "totalStorage"))]
    pub total_storage: Long,
    pub display: Option<DisplayInfo>,
    pub android: Option<AndroidExtras>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct Temperature {
    pub label: String,
    pub celsius: f64,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct DeviceStatus {
    #[cfg_attr(feature = "graphql", graphql(name = "uptimeSec"))]
    pub uptime_sec: Long,
    #[cfg_attr(feature = "graphql", graphql(name = "batteryLevel"))]
    pub battery_level: Option<i32>,
    pub charging: bool,
    pub temperatures: Vec<Temperature>,
    #[cfg_attr(feature = "graphql", graphql(name = "cpuUsage"))]
    pub cpu_usage: f64,
    #[cfg_attr(feature = "graphql", graphql(name = "memoryAvailable"))]
    pub memory_available: Option<Long>,
    #[cfg_attr(feature = "graphql", graphql(name = "storageAvailable"))]
    pub storage_available: Long,
}

impl DeviceType {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceType::Computer => "COMPUTER",
            DeviceType::Phone => "PHONE",
            DeviceType::Tablet => "TABLET",
            DeviceType::Tv => "TV",
            DeviceType::Nas => "NAS",
            DeviceType::Other => "OTHER",
        }
    }
}

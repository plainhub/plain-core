use plain_libs::protocols::mdns::records::MdnsServiceInfo;

pub const PLAINAPP_SERVICE_TYPE: &str = "_plainapp._tcp.local";

/// Builds the advertised mDNS service for this device. TXT keys are the
/// PlainApp wire schema: id / dv / ver / pf (plus aw / ar Wi-Fi-Aware flags,
/// supplied by the caller).
pub fn build_service_info(
    instance_name: &str,
    hostname: &str,
    port: u16,
    id: &str,
    device_type: &str,
    version: &str,
    platform: &str,
    ips: Vec<String>,
    aware_supported: bool,
    aware_running: bool,
) -> MdnsServiceInfo {
    MdnsServiceInfo {
        instance_name: instance_name.to_string(),
        service_type: PLAINAPP_SERVICE_TYPE.to_string(),
        target_hostname: hostname.to_string(),
        port,
        txt_records: vec![
            format!("id={id}"),
            format!("dv={device_type}"),
            format!("ver={version}"),
            format!("pf={platform}"),
            format!("aw={}", u8::from(aware_supported)),
            format!("ar={}", u8::from(aware_running)),
        ],
        ips,
    }
}

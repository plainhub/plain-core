use plain_rs::services::casting::sender::types::DiscoveredDevice;
use plain_rs::services::casting::sender::types::UpnpService;
use plain_rs::services::casting::sender::xml_sax::Event;
use plain_rs::services::casting::sender::xml_sax::Reader;

pub fn parse_device_desc(body: &[u8]) -> DiscoveredDevice {
    let mut dev = DiscoveredDevice::default();
    let mut r = Reader::new(body);

    // State: which `<X>` we are currently inside. We descend into
    // `<device>...</device>` and collect its immediate children's text.
    let mut path: Vec<String> = Vec::new();
    // When we see `<service>` (only meaningful inside `<serviceList>`), we
    // start collecting into a local UpnpService.
    let mut in_service = false;
    let mut cur_service = UpnpService::default();

    while let Some(ev) = r.next_event() {
        match ev {
            Event::Start { name } => {
                if name == "service" {
                    in_service = true;
                    cur_service = UpnpService::default();
                } else {
                    path.push(name);
                }
            }
            Event::End { name } => {
                if name == "service" {
                    if cur_service.service_type.contains("AVTransport:1")
                        || cur_service.service_id.contains("AVTransport")
                    {
                        dev.has_av_transport = true;
                        dev.av_transport = UpnpService {
                            service_type: cur_service.service_type.trim().to_string(),
                            service_id: cur_service.service_id.trim().to_string(),
                            control_url: cur_service.control_url.trim().to_string(),
                            event_sub_url: cur_service.event_sub_url.trim().to_string(),
                            scpd_url: cur_service.scpd_url.trim().to_string(),
                        };
                    }
                    in_service = false;
                } else {
                    path.pop();
                }
            }
            Event::Text { content } => {
                let field = path.last().cloned().unwrap_or_default();
                if in_service {
                    match field.as_str() {
                        "serviceType" => cur_service.service_type = content,
                        "serviceId" => cur_service.service_id = content,
                        "controlURL" => cur_service.control_url = content,
                        "eventSubURL" => cur_service.event_sub_url = content,
                        "SCPDURL" => cur_service.scpd_url = content,
                        _ => {}
                    }
                } else if path.iter().any(|p| p == "device") {
                    match field.as_str() {
                        "friendlyName" => dev.friendly_name = content,
                        "manufacturer" => dev.manufacturer = content,
                        "modelName" => dev.model_name = content,
                        "UDN" => dev.udn = content,
                        _ => {}
                    }
                }
            }
        }
    }
    dev
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/casting/sender/desc.rs"]
mod tests;
